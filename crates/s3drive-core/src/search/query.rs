//! 検索条件から SQL を組み立てて実行する（04 §10.2、06 §4.4）。

use std::sync::Arc;

use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Utc};
use rusqlite::types::Value;

use super::indexer::IndexRunner;
use crate::Core;
use crate::error::CoreResult;
use crate::jobs::{NullSink, ProgressSink};
use crate::model::{
    DateFilter, Entry, FileKind, IndexEvent, IndexState, JobId, RestoreState, SearchEntry,
    SearchQuery, SearchResult, SortKey, StorageClass,
};
use crate::util::{key, time};

/// 名前だけで検索したときに含めるフォルダの上限。
const FOLDER_LIMIT: i64 = 200;

pub struct BuiltQuery {
    pub where_sql: String,
    pub params: Vec<Value>,
    pub order_sql: String,
}

/// ファイルの検索条件を SQL にする。`now` は期間の計算に使う（ローカル時刻の「今年」）。
pub fn build_sql(connection_id: &str, q: &SearchQuery, now: DateTime<Local>) -> BuiltQuery {
    let mut clauses = vec![
        "o.connection_id = ?".to_string(),
        "o.is_marker = 0".to_string(),
    ];
    let mut params: Vec<Value> = vec![Value::Text(connection_id.to_string())];

    let text = key::normalize_for_search(q.text.trim());
    if !text.is_empty() {
        if text.chars().count() >= 3 {
            // 3 文字以上は FTS5（trigram）
            clauses
                .push("o.id IN (SELECT rowid FROM objects_fts WHERE objects_fts MATCH ?)".into());
            params.push(Value::Text(format!("\"{}\"", text.replace('"', "\"\""))));
        } else {
            // 2 文字以下は部分一致（LIKE の特殊文字を避けるため instr を使う）
            clauses.push("instr(o.name_norm, ?) > 0".into());
            params.push(Value::Text(text));
        }
    }
    if let Some(kind) = q.kind {
        if kind == FileKind::Doc {
            let known = FileKind::all_known_extensions();
            clauses.push(format!(
                "(o.ext IS NULL OR o.ext NOT IN ({}))",
                vec!["?"; known.len()].join(", ")
            ));
            params.extend(known.iter().map(|e| Value::Text((*e).to_string())));
        } else {
            let exts = kind.extensions();
            clauses.push(format!("o.ext IN ({})", vec!["?"; exts.len()].join(", ")));
            params.extend(exts.iter().map(|e| Value::Text((*e).to_string())));
        }
    }
    if let Some(ext) = q
        .ext
        .as_deref()
        .map(|e| e.trim().trim_start_matches('.').to_lowercase())
        && !ext.is_empty()
    {
        clauses.push("o.ext = ?".into());
        params.push(Value::Text(ext));
    }
    if let Some(size) = q.size {
        let (min, max) = size.range();
        if let Some(min) = min {
            clauses.push("o.size >= ?".into());
            params.push(Value::Integer(min as i64));
        }
        if let Some(max) = max {
            clauses.push("o.size < ?".into());
            params.push(Value::Integer(max as i64));
        }
    }
    if let Some(date) = q.date {
        let since: DateTime<Utc> = match date {
            DateFilter::Days7 => (now - Duration::days(7)).with_timezone(&Utc),
            DateFilter::Days30 => (now - Duration::days(30)).with_timezone(&Utc),
            DateFilter::Year => Local
                .with_ymd_and_hms(now.year(), 1, 1, 0, 0, 0)
                .single()
                .unwrap_or(now)
                .with_timezone(&Utc),
        };
        clauses.push("o.last_modified >= ?".into());
        params.push(Value::Text(time::to_rfc3339(since)));
    }
    if let Some(class) = q.storage_class {
        clauses.push("o.storage_class = ?".into());
        params.push(Value::Text(class.as_s3().to_string()));
    }

    // 並び順は一覧と同じ規則（名前は自然順。同じ値は名前順）にする（04 §10.2）
    let dir = if q.sort.dir < 0 { "DESC" } else { "ASC" };
    let by_name = "o.name_norm COLLATE NATURAL_ORDER";
    let order_sql = match q.sort.key {
        SortKey::Name => format!("{by_name} {dir}, o.key"),
        SortKey::Modified => format!("o.last_modified {dir}, {by_name}"),
        SortKey::Size => format!("o.size {dir}, {by_name}"),
        SortKey::StorageClass => format!("o.storage_class {dir}, {by_name}"),
    };
    BuiltQuery {
        where_sql: clauses.join(" AND "),
        params,
        order_sql,
    }
}

fn file_entry(
    key: String,
    name: String,
    size: i64,
    last_modified: String,
    etag: Option<String>,
    class: String,
) -> Entry {
    let storage_class = StorageClass::from_s3(Some(&class));
    Entry::File {
        key,
        name,
        size: size.max(0) as u64,
        last_modified,
        etag: etag.unwrap_or_default(),
        restore: if storage_class.is_archive() {
            RestoreState::Archived
        } else {
            RestoreState::NotArchived
        },
        storage_class,
        deleted: false,
    }
}

/// インデックスを検索する（同期。`spawn_blocking` 上で呼ぶ）。
pub(crate) fn run_query(
    conn: &rusqlite::Connection,
    connection_id: &str,
    q: &SearchQuery,
) -> rusqlite::Result<(Vec<SearchEntry>, u64)> {
    let text = key::normalize_for_search(q.text.trim());
    if text.is_empty() && !q.has_filters() {
        return Ok((Vec::new(), 0));
    }
    let built = build_sql(connection_id, q, Local::now());
    let total: i64 = conn.query_row(
        &format!(
            "SELECT count(*) FROM objects AS o WHERE {}",
            built.where_sql
        ),
        rusqlite::params_from_iter(built.params.iter()),
        |r| r.get(0),
    )?;

    let mut entries = Vec::new();
    let mut folder_total = 0i64;
    // 名前だけで検索し、フィルタを指定していない場合は、名前が一致するフォルダも含める（DS の仕様）。
    // フォルダは一覧と同じく先頭に置くため、最初のページ（offset = 0）にだけ最大 FOLDER_LIMIT 件を含め、
    // offset・limit はファイルだけに適用する。件数も返したフォルダの数だけを加える。
    if !text.is_empty() && !q.has_filters() {
        let matched: i64 = conn.query_row(
            "SELECT count(*) FROM prefixes WHERE connection_id = ?1 AND instr(name_norm, ?2) > 0",
            rusqlite::params![connection_id, text],
            |r| r.get(0),
        )?;
        folder_total = matched.min(FOLDER_LIMIT);
        if q.offset == 0 {
            let mut stmt = conn.prepare(
                "SELECT p.prefix, p.name, p.parent, m.last_modified FROM prefixes AS p
                 LEFT JOIN objects AS m ON m.connection_id = p.connection_id AND m.key = p.prefix
                 WHERE p.connection_id = ?1 AND instr(p.name_norm, ?2) > 0
                 ORDER BY p.name_norm COLLATE NATURAL_ORDER LIMIT ?3",
            )?;
            let rows =
                stmt.query_map(rusqlite::params![connection_id, text, FOLDER_LIMIT], |r| {
                    Ok(SearchEntry {
                        entry: Entry::Folder {
                            key: r.get(0)?,
                            name: r.get(1)?,
                            last_modified: r.get(3)?,
                            deleted: false,
                        },
                        parent: r.get(2)?,
                    })
                })?;
            for row in rows {
                entries.push(row?);
            }
        }
    }

    let mut params = built.params.clone();
    params.push(Value::Integer(q.limit.clamp(1, 1000) as i64));
    params.push(Value::Integer(q.offset as i64));
    let mut stmt = conn.prepare(&format!(
        "SELECT o.key, o.name, o.parent, o.size, o.last_modified, o.etag, o.storage_class
         FROM objects AS o WHERE {} ORDER BY {} LIMIT ? OFFSET ?",
        built.where_sql, built.order_sql
    ))?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |r| {
        Ok(SearchEntry {
            entry: file_entry(
                r.get(0)?,
                r.get(1)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ),
            parent: r.get(2)?,
        })
    })?;
    for row in rows {
        entries.push(row?);
    }
    Ok((entries, (total + folder_total) as u64))
}

pub struct SearchService<'a> {
    core: &'a Core,
}

impl Core {
    pub fn search(&self) -> SearchService<'_> {
        SearchService { core: self }
    }
}

impl SearchService<'_> {
    /// 現在のインデックスから結果を即時に返す。インデックスがない・古い場合は背景で走査する（04 §10.1）。
    pub async fn query(&self, connection_id: &str, q: SearchQuery) -> CoreResult<SearchResult> {
        let ctx = self.core.ctx(connection_id).await?;
        let auto = self
            .core
            .0
            .settings
            .read(|f| f.settings.search.auto_refresh_minutes);
        let runner = IndexRunner::new(self.core);
        let mut index = self.status(connection_id).await?;
        let needs_scan = match index.state {
            IndexState::None => true,
            IndexState::Stale => auto > 0,
            _ => false,
        };
        if needs_scan {
            runner.start(ctx, Arc::new(NullSink));
            index = self.status(connection_id).await?;
        }
        let cid = connection_id.to_string();
        let (entries, total) = self
            .core
            .0
            .db
            .run(move |c| Ok(run_query(c, &cid, &q)?))
            .await?;
        Ok(SearchResult {
            entries,
            total,
            index,
        })
    }

    pub async fn status(&self, connection_id: &str) -> CoreResult<crate::model::IndexStatus> {
        let auto = self
            .core
            .0
            .settings
            .read(|f| f.settings.search.auto_refresh_minutes);
        let progress = IndexRunner::new(self.core).progress(connection_id);
        super::status(&self.core.0.db, connection_id, auto, progress).await
    }

    /// 全件走査をやり直す（設定「再構築」）。走査中なら実行中のジョブの ID を返し、その進捗と結果を `sink` にも通知する。
    pub async fn rebuild(
        &self,
        connection_id: &str,
        sink: Arc<dyn ProgressSink<IndexEvent>>,
    ) -> CoreResult<JobId> {
        let ctx = self.core.ctx(connection_id).await?;
        Ok(IndexRunner::new(self.core).start(ctx, sink))
    }

    pub async fn delete(&self, connection_id: &str) -> CoreResult<()> {
        self.core.ctx(connection_id).await?;
        IndexRunner::new(self.core).cancel(connection_id);
        super::delete_index(&self.core.0.db, connection_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::seed;
    use super::*;
    use crate::model::{SizeFilter, Sort};
    use crate::store::Db;

    async fn db() -> (Db, crate::store::db::tempfile_guard::TempDir) {
        let (db, dir) = Db::open_temp().unwrap();
        let recent = time::to_rfc3339(time::now() - Duration::days(2));
        let old = "2020-01-01T00:00:00Z";
        seed(
            &db,
            "c",
            &[
                ("projects/", 0, old, StorageClass::Standard),
                (
                    "projects/2026/report-q3.pdf",
                    4_200_000,
                    &recent,
                    StorageClass::Standard,
                ),
                (
                    "projects/2026/Ｒｅｐｏｒｔ-draft.PDF",
                    900_000,
                    old,
                    StorageClass::StandardIa,
                ),
                (
                    "projects/2026/ingest.rs",
                    18_000,
                    &recent,
                    StorageClass::Standard,
                ),
                (
                    "backups/db.sql.gz",
                    4_800_000_000,
                    old,
                    StorageClass::DeepArchive,
                ),
                ("reports/README", 10, old, StorageClass::Standard),
                (
                    "photos/IMG_2041.heic",
                    3_100_000,
                    &recent,
                    StorageClass::Standard,
                ),
            ],
        )
        .await;
        (db, dir)
    }

    fn keys(entries: &[SearchEntry]) -> Vec<String> {
        entries.iter().map(|e| e.entry.key().to_string()).collect()
    }

    async fn search(db: &Db, q: SearchQuery) -> (Vec<SearchEntry>, u64) {
        db.run(move |c| Ok(run_query(c, "c", &q)?)).await.unwrap()
    }

    #[tokio::test]
    async fn matches_names_with_width_and_case_folding() {
        let (db, _d) = db().await;
        let (entries, total) = search(&db, SearchQuery::text("report")).await;
        // フォルダ「reports/」と 2 つの PDF
        assert_eq!(total, 3);
        assert_eq!(entries[0].entry.key(), "reports/");
        assert!(keys(&entries).contains(&"projects/2026/Ｒｅｐｏｒｔ-draft.PDF".to_string()));
        assert_eq!(entries[1].parent, "projects/2026/");
    }

    #[tokio::test]
    async fn short_terms_use_substring_matching() {
        let (db, _d) = db().await;
        let (entries, _) = search(&db, SearchQuery::text("rs")).await;
        assert!(keys(&entries).contains(&"projects/2026/ingest.rs".to_string()));
    }

    #[tokio::test]
    async fn filters_exclude_folders_and_apply_conditions() {
        let (db, _d) = db().await;
        let mut q = SearchQuery::text("");
        q.kind = Some(FileKind::Pdf);
        let (entries, _) = search(&db, q.clone()).await;
        assert_eq!(entries.len(), 2);

        q.kind = None;
        q.ext = Some(".GZ".into());
        assert_eq!(keys(&search(&db, q.clone()).await.0), ["backups/db.sql.gz"]);

        q.ext = None;
        q.size = Some(SizeFilter::Gt100);
        assert_eq!(keys(&search(&db, q.clone()).await.0), ["backups/db.sql.gz"]);

        q.size = None;
        q.date = Some(DateFilter::Days7);
        assert_eq!(search(&db, q.clone()).await.0.len(), 3);

        q.date = None;
        q.storage_class = Some(StorageClass::StandardIa);
        assert_eq!(search(&db, q.clone()).await.0.len(), 1);

        q.storage_class = None;
        q.kind = Some(FileKind::Doc);
        assert_eq!(keys(&search(&db, q).await.0), ["reports/README"]);
    }

    #[tokio::test]
    async fn sorts_and_pages_results() {
        let (db, _d) = db().await;
        let mut q = SearchQuery::text("");
        q.size = Some(SizeFilter::Lt1);
        q.sort = Sort {
            key: SortKey::Size,
            dir: -1,
        };
        let (entries, total) = search(&db, q.clone()).await;
        assert_eq!(total, 3);
        assert_eq!(entries[0].entry.name(), "Ｒｅｐｏｒｔ-draft.PDF");
        q.offset = 2;
        q.limit = 10;
        assert_eq!(keys(&search(&db, q).await.0), ["reports/README"]);
    }

    #[tokio::test]
    async fn sorts_names_naturally_and_pages_only_files() {
        let (db, _d) = Db::open_temp().unwrap();
        let old = "2020-01-01T00:00:00Z";
        seed(
            &db,
            "c",
            &[
                ("log10/", 0, old, StorageClass::Standard),
                ("log2/", 0, old, StorageClass::Standard),
                ("a/log10.txt", 1, old, StorageClass::Standard),
                ("a/log2.txt", 1, old, StorageClass::Standard),
                ("a/log1.txt", 1, old, StorageClass::Standard),
            ],
        )
        .await;
        let mut q = SearchQuery::text("log");
        q.limit = 2;
        let (entries, total) = search(&db, q.clone()).await;
        // フォルダ 2 件は先頭に（自然順）、ファイルは limit 件まで（自然順）
        assert_eq!(total, 5);
        assert_eq!(
            keys(&entries),
            ["log2/", "log10/", "a/log1.txt", "a/log2.txt"]
        );
        // 2 ページ目はファイルの続きだけ
        q.offset = 2;
        assert_eq!(keys(&search(&db, q).await.0), ["a/log10.txt"]);
    }

    #[tokio::test]
    async fn empty_query_returns_nothing() {
        let (db, _d) = db().await;
        assert_eq!(search(&db, SearchQuery::text("  ")).await.1, 0);
    }

    #[test]
    fn escapes_quotes_in_fts_phrases() {
        let mut q = SearchQuery::text("a\"bc");
        q.sort = Sort::default();
        let built = build_sql("c", &q, Local::now());
        assert_eq!(built.params[1], Value::Text("\"a\"\"bc\"".into()));
    }
}

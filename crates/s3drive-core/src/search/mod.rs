//! 検索インデックス（D5、04 §10、06 §4）。
//!
//! 接続ごとにバケット内の全キーを SQLite に保存し、名前（FTS5 trigram）と各フィルタで検索する。

mod indexer;
mod query;

use std::collections::{BTreeSet, HashMap};

use rusqlite::{OptionalExtension, Transaction, params};

use crate::error::CoreResult;
use crate::model::{FolderSummary, IndexState, IndexStatus, StorageClass};
use crate::store::Db;
use crate::util::{key, time};

pub use indexer::{IndexBuilds, IndexListener};
pub use query::{SearchService, build_sql};

/// インデックスに書き込むオブジェクト。
#[derive(Debug, Clone)]
pub struct IndexedObject {
    pub key: String,
    pub size: u64,
    pub last_modified: String,
    pub etag: Option<String>,
    pub storage_class: StorageClass,
}

/// 1 行を書き込む（同じキーは上書き）。
pub(crate) fn write_object(
    tx: &Transaction<'_>,
    connection_id: &str,
    o: &IndexedObject,
    generation: i64,
) -> rusqlite::Result<()> {
    let name = key::nfc(key::base_name(&o.key));
    let is_marker = key::is_folder_key(&o.key);
    tx.execute(
        "INSERT INTO objects (connection_id, key, name, name_norm, ext, parent, size, last_modified, etag, storage_class, is_marker, generation)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT (connection_id, key) DO UPDATE SET
           name = excluded.name, name_norm = excluded.name_norm, ext = excluded.ext, parent = excluded.parent,
           size = excluded.size, last_modified = excluded.last_modified, etag = excluded.etag,
           storage_class = excluded.storage_class, is_marker = excluded.is_marker, generation = excluded.generation",
        params![
            connection_id,
            o.key,
            name,
            key::normalize_for_search(&name),
            if is_marker { None } else { key::extension(&name) },
            key::parent_prefix(&o.key),
            o.size as i64,
            o.last_modified,
            o.etag,
            o.storage_class.as_s3(),
            is_marker,
            generation,
        ],
    )?;
    Ok(())
}

/// キーの親プレフィックスをすべて `prefixes` に書き込む（フォルダ名の検索用）。
pub(crate) fn write_prefixes(
    tx: &Transaction<'_>,
    connection_id: &str,
    object_key: &str,
    generation: i64,
    seen: &mut BTreeSet<String>,
) -> rusqlite::Result<()> {
    let mut prefix = if key::is_folder_key(object_key) {
        object_key.to_string()
    } else {
        key::parent_prefix(object_key).to_string()
    };
    while !prefix.is_empty() {
        if !seen.insert(prefix.clone()) {
            break;
        }
        let name = key::nfc(key::base_name(&prefix));
        tx.execute(
            "INSERT INTO prefixes (connection_id, prefix, name, name_norm, parent, generation) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (connection_id, prefix) DO UPDATE SET generation = excluded.generation",
            params![
                connection_id,
                prefix,
                name,
                key::normalize_for_search(&name),
                key::parent_prefix(&prefix),
                generation
            ],
        )?;
        prefix = key::parent_prefix(&prefix).to_string();
    }
    Ok(())
}

fn current_generation(tx: &Transaction<'_>, connection_id: &str) -> rusqlite::Result<Option<i64>> {
    tx.query_row(
        "SELECT generation FROM index_state WHERE connection_id = ?1",
        [connection_id],
        |r| r.get(0),
    )
    .optional()
}

fn refresh_counts(tx: &Transaction<'_>, connection_id: &str) -> rusqlite::Result<()> {
    tx.execute(
        "UPDATE index_state SET
           object_count = (SELECT count(*) FROM objects WHERE connection_id = ?1 AND is_marker = 0),
           total_bytes = (SELECT coalesce(sum(size), 0) FROM objects WHERE connection_id = ?1)
         WHERE connection_id = ?1",
        [connection_id],
    )?;
    Ok(())
}

/// 件数と合計サイズを差分で更新する。
///
/// 変更のたびに [`refresh_counts`] で全行を数え直すと、アップロード 1 ファイルごとに接続の全行を読むことになる。
/// 走査の完了時には数え直すため、差分の誤差は残らない。
fn adjust_counts(
    tx: &Transaction<'_>,
    connection_id: &str,
    objects: i64,
    bytes: i64,
) -> rusqlite::Result<()> {
    if objects == 0 && bytes == 0 {
        return Ok(());
    }
    tx.execute(
        "UPDATE index_state SET object_count = max(0, object_count + ?2), total_bytes = max(0, total_bytes + ?3)
         WHERE connection_id = ?1",
        params![connection_id, objects, bytes],
    )?;
    Ok(())
}

/// `prefix` 配下のキーに絞る条件（`?1` は接続 ID）と、その引数。
///
/// `substr(key, 1, length(?)) = ?` はインデックスを使えず接続の全行を読むため、(connection_id, key) の
/// インデックスで引ける範囲の条件にする。
fn under_prefix(connection_id: &str, prefix: &str) -> (&'static str, Vec<String>) {
    let cid = connection_id.to_string();
    if prefix.is_empty() {
        return ("connection_id = ?1", vec![cid]);
    }
    match key::prefix_upper_bound(prefix) {
        Some(upper) => (
            "connection_id = ?1 AND key >= ?2 AND key < ?3",
            vec![cid, prefix.to_string(), upper],
        ),
        None => (
            "connection_id = ?1 AND substr(key, 1, length(?2)) = ?2",
            vec![cid, prefix.to_string()],
        ),
    }
}

/// アプリ自身の変更（アップロード・移動・クラス変更など）を即時に反映する。インデックスがなければ何もしない。
pub async fn upsert(db: &Db, connection_id: &str, objects: Vec<IndexedObject>) -> CoreResult<()> {
    if objects.is_empty() {
        return Ok(());
    }
    let cid = connection_id.to_string();
    db.run(move |c| {
        let tx = c.transaction()?;
        let Some(generation) = current_generation(&tx, &cid)? else {
            return Ok(());
        };
        let mut seen = BTreeSet::new();
        let (mut count, mut bytes) = (0i64, 0i64);
        for o in &objects {
            let previous: Option<(i64, bool)> = tx
                .query_row(
                    "SELECT size, is_marker FROM objects WHERE connection_id = ?1 AND key = ?2",
                    params![cid, o.key],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            write_object(&tx, &cid, o, generation)?;
            write_prefixes(&tx, &cid, &o.key, generation, &mut seen)?;
            let (old_size, old_counted) = match previous {
                Some((size, is_marker)) => (size, !is_marker),
                None => (0, false),
            };
            bytes += o.size as i64 - old_size;
            count += i64::from(!key::is_folder_key(&o.key)) - i64::from(old_counted);
        }
        adjust_counts(&tx, &cid, count, bytes)?;
        tx.commit()?;
        Ok(())
    })
    .await
}

/// 削除・移動したキーを取り除く。中身がなくなったフォルダも取り除く。
pub async fn remove(db: &Db, connection_id: &str, keys: Vec<String>) -> CoreResult<()> {
    if keys.is_empty() {
        return Ok(());
    }
    let cid = connection_id.to_string();
    db.run(move |c| {
        let tx = c.transaction()?;
        if current_generation(&tx, &cid)?.is_none() {
            return Ok(());
        }
        let mut parents = BTreeSet::new();
        let (mut count, mut bytes) = (0i64, 0i64);
        for k in &keys {
            let removed: Option<(i64, bool)> = tx
                .query_row(
                    "DELETE FROM objects WHERE connection_id = ?1 AND key = ?2 RETURNING size, is_marker",
                    params![cid, k],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            if let Some((size, is_marker)) = removed {
                bytes -= size;
                count -= i64::from(!is_marker);
            }
            let mut p = if key::is_folder_key(k) { k.clone() } else { key::parent_prefix(k).to_string() };
            while !p.is_empty() {
                parents.insert(p.clone());
                p = key::parent_prefix(&p).to_string();
            }
        }
        // 深い階層から順に、配下にオブジェクトがなくなったプレフィックスを消す
        for p in parents.iter().rev() {
            let (filter, args) = under_prefix(&cid, p);
            let occupied: bool = tx.query_row(
                &format!("SELECT EXISTS (SELECT 1 FROM objects WHERE {filter})"),
                rusqlite::params_from_iter(args.iter()),
                |r| r.get(0),
            )?;
            if !occupied {
                tx.execute("DELETE FROM prefixes WHERE connection_id = ?1 AND prefix = ?2", params![cid, p])?;
            }
        }
        adjust_counts(&tx, &cid, count, bytes)?;
        tx.commit()?;
        Ok(())
    })
    .await
}

/// フォルダマーカーの日時（一覧のフォルダの更新日に使う。04 §3.2）。
pub async fn folder_marker_dates(
    db: &Db,
    connection_id: &str,
    parent: &str,
) -> CoreResult<HashMap<String, String>> {
    let (cid, parent) = (connection_id.to_string(), parent.to_string());
    db.run(move |c| {
        let mut stmt = c.prepare_cached(
            "SELECT key, last_modified FROM objects WHERE connection_id = ?1 AND parent = ?2 AND is_marker = 1",
        )?;
        let rows = stmt
            .query_map(params![cid, parent], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<HashMap<_, _>, _>>()?;
        Ok(rows)
    })
    .await
}

/// 全件走査を 1 回以上終えたインデックスがあるか。`idle` なら、走査中でないことも求める。
///
/// 初回の走査が失敗・キャンセルで終わると、途中までの行だけが残る。それを完成したインデックスとして
/// 集計に使うと、フォルダの合計サイズやクラス別の容量が実際より小さくなる。
fn fully_scanned(
    c: &rusqlite::Connection,
    connection_id: &str,
    idle: bool,
) -> rusqlite::Result<bool> {
    let row: Option<(String, Option<String>)> = c
        .query_row(
            "SELECT status, last_full_scan_at FROM index_state WHERE connection_id = ?1",
            [connection_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(row.is_some_and(|(status, last_scan)| last_scan.is_some() && (!idle || status == "ready")))
}

/// インデックスが完成していれば、そこからフォルダの項目数と合計サイズを求める。
pub async fn folder_summary(
    db: &Db,
    connection_id: &str,
    prefix: &str,
) -> CoreResult<Option<FolderSummary>> {
    let (cid, prefix) = (connection_id.to_string(), prefix.to_string());
    db.run(move |c| {
        if !fully_scanned(c, &cid, true)? {
            return Ok(None);
        }
        let files: i64 = c.query_row(
            "SELECT count(*) FROM objects WHERE connection_id = ?1 AND parent = ?2 AND is_marker = 0",
            params![cid, prefix],
            |r| r.get(0),
        )?;
        let folders: i64 = c.query_row(
            "SELECT count(*) FROM prefixes WHERE connection_id = ?1 AND parent = ?2",
            params![cid, prefix],
            |r| r.get(0),
        )?;
        let (filter, args) = under_prefix(&cid, &prefix);
        let total: i64 = c.query_row(
            &format!("SELECT coalesce(sum(size), 0) FROM objects WHERE {filter}"),
            rusqlite::params_from_iter(args.iter()),
            |r| r.get(0),
        )?;
        Ok(Some(FolderSummary {
            item_count: (files + folders) as u64,
            total_bytes: total as u64,
            truncated: false,
        }))
    })
    .await
}

/// インデックスの状態。`auto_refresh_minutes` を過ぎていれば `stale`。
pub async fn status(
    db: &Db,
    connection_id: &str,
    auto_refresh_minutes: u32,
    progress: Option<u64>,
) -> CoreResult<IndexStatus> {
    let cid = connection_id.to_string();
    let size = db.size_bytes();
    let row: Option<(String, i64, Option<String>)> = db
        .run(move |c| {
            Ok(c.query_row(
                "SELECT status, object_count, last_full_scan_at FROM index_state WHERE connection_id = ?1",
                [cid],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?)
        })
        .await?;
    let Some((state, count, last_scan)) = row else {
        return Ok(IndexStatus {
            progress,
            ..IndexStatus::none()
        });
    };
    // 全件走査を一度も終えていない（初回が失敗・キャンセルで終わった）ものは、自動更新の設定に関係なく
    // 古いものとして扱い、次の検索で走査し直す
    let scanned_at = last_scan.as_deref().and_then(time::parse_rfc3339);
    let stale = match scanned_at {
        None => true,
        Some(t) => {
            auto_refresh_minutes > 0
                && (time::now() - t).num_minutes() >= auto_refresh_minutes as i64
        }
    };
    let state = match (progress.is_some() || state == "building", stale) {
        (true, _) => IndexState::Building,
        (false, true) => IndexState::Stale,
        (false, false) => IndexState::Ready,
    };
    Ok(IndexStatus {
        state,
        object_count: count as u64,
        last_scan_at: last_scan,
        size_bytes: size,
        progress,
    })
}

/// インデックスの削除（設定「削除」）。
pub async fn delete_index(db: &Db, connection_id: &str) -> CoreResult<()> {
    let cid = connection_id.to_string();
    db.run(move |c| {
        let tx = c.transaction()?;
        for table in ["objects", "prefixes", "index_state"] {
            tx.execute(
                &format!("DELETE FROM {table} WHERE connection_id = ?1"),
                [&cid],
            )?;
        }
        tx.commit()?;
        Ok(())
    })
    .await
}

/// インデックスから、クラス別の容量とオブジェクト数を集計する（現行バージョンのみ。04 §12.2）。
pub async fn class_totals(
    db: &Db,
    connection_id: &str,
) -> CoreResult<Option<(HashMap<StorageClass, u64>, u64)>> {
    let cid = connection_id.to_string();
    db.run(move |c| {
        // 再走査中は前回までの行が残っているため使う。初回の走査を終えていないものは使わない
        if !fully_scanned(c, &cid, false)? {
            return Ok(None);
        }
        let mut stmt = c.prepare(
            "SELECT storage_class, sum(size), sum(CASE WHEN is_marker = 0 THEN 1 ELSE 0 END)
             FROM objects WHERE connection_id = ?1 GROUP BY storage_class",
        )?;
        let mut totals = HashMap::new();
        let mut count = 0u64;
        let rows = stmt.query_map([&cid], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        for row in rows {
            let (class, bytes, n) = row?;
            *totals
                .entry(StorageClass::from_s3(Some(&class)))
                .or_insert(0) += bytes as u64;
            count += n as u64;
        }
        Ok(Some((totals, count)))
    })
    .await
}

/// ベンチマーク用（`benches/search.rs`。09 §5）。インデックスを直接作り、同期で検索する。
#[cfg(feature = "bench")]
#[doc(hidden)]
pub mod bench_support {
    use super::*;
    use crate::model::{SearchEntry, SearchQuery};

    /// 完成したインデックス（世代 1）を 1 つのトランザクションで作る。
    pub fn seed(conn: &mut rusqlite::Connection, connection_id: &str, objects: &[IndexedObject]) {
        let tx = conn.transaction().unwrap();
        tx.execute(
            "INSERT INTO index_state (connection_id, status, generation, last_full_scan_at) VALUES (?1, 'ready', 1, ?2)",
            params![connection_id, time::now_rfc3339()],
        )
        .unwrap();
        let mut seen = BTreeSet::new();
        for o in objects {
            write_object(&tx, connection_id, o, 1).unwrap();
            write_prefixes(&tx, connection_id, &o.key, 1, &mut seen).unwrap();
        }
        refresh_counts(&tx, connection_id).unwrap();
        tx.commit().unwrap();
    }

    pub fn query(
        conn: &rusqlite::Connection,
        connection_id: &str,
        q: &SearchQuery,
    ) -> (Vec<SearchEntry>, u64) {
        query::run_query(conn, connection_id, q).unwrap()
    }

    /// `Db::open` と同じ設定（WAL・自然順の照合順序）で接続を開く。
    pub fn open(path: &std::path::Path) -> rusqlite::Connection {
        crate::store::db::open_for_bench(path)
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    /// テスト用に、世代 1 の完成したインデックスを作る。
    pub async fn seed(db: &Db, connection_id: &str, objects: &[(&str, u64, &str, StorageClass)]) {
        let cid = connection_id.to_string();
        let objects: Vec<IndexedObject> = objects
            .iter()
            .map(|(k, size, modified, class)| IndexedObject {
                key: k.to_string(),
                size: *size,
                last_modified: modified.to_string(),
                etag: None,
                storage_class: *class,
            })
            .collect();
        db.run(move |c| {
            let tx = c.transaction()?;
            tx.execute(
                "INSERT INTO index_state (connection_id, status, generation, last_full_scan_at) VALUES (?1, 'ready', 1, ?2)",
                params![cid, time::now_rfc3339()],
            )?;
            let mut seen = BTreeSet::new();
            for o in &objects {
                write_object(&tx, &cid, o, 1)?;
                write_prefixes(&tx, &cid, &o.key, 1, &mut seen)?;
            }
            refresh_counts(&tx, &cid)?;
            tx.commit()?;
            Ok(())
        })
        .await
        .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::seed;
    use super::*;

    #[tokio::test]
    async fn applies_changes_only_when_an_index_exists() {
        let (db, _dir) = Db::open_temp().unwrap();
        let obj = IndexedObject {
            key: "a/b.txt".into(),
            size: 1,
            last_modified: "2026-01-01T00:00:00Z".into(),
            etag: None,
            storage_class: StorageClass::Standard,
        };
        upsert(&db, "c", vec![obj.clone()]).await.unwrap();
        assert!(folder_summary(&db, "c", "").await.unwrap().is_none());

        seed(
            &db,
            "c",
            &[("x/", 0, "2026-01-01T00:00:00Z", StorageClass::Standard)],
        )
        .await;
        upsert(&db, "c", vec![obj]).await.unwrap();
        let root = folder_summary(&db, "c", "").await.unwrap().unwrap();
        assert_eq!(root.item_count, 2); // a/ と x/
        assert_eq!(root.total_bytes, 1);

        remove(&db, "c", vec!["a/b.txt".into()]).await.unwrap();
        let root = folder_summary(&db, "c", "").await.unwrap().unwrap();
        assert_eq!(root.item_count, 1); // a/ は空になったので消える
    }

    async fn counts(db: &Db) -> (i64, i64) {
        db.run(|c| {
            Ok(c.query_row(
                "SELECT object_count, total_bytes FROM index_state WHERE connection_id = 'c'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?)
        })
        .await
        .unwrap()
    }

    fn object(key: &str, size: u64) -> IndexedObject {
        IndexedObject {
            key: key.into(),
            size,
            last_modified: "2026-01-01T00:00:00Z".into(),
            etag: None,
            storage_class: StorageClass::Standard,
        }
    }

    #[tokio::test]
    async fn keeps_counts_in_step_with_changes() {
        let (db, _dir) = Db::open_temp().unwrap();
        let at = "2026-01-01T00:00:00Z";
        seed(
            &db,
            "c",
            &[
                ("a/x.txt", 10, at, StorageClass::Standard),
                ("a/y.txt", 5, at, StorageClass::Standard),
                ("b/", 0, at, StorageClass::Standard),
            ],
        )
        .await;
        assert_eq!(counts(&db).await, (2, 15));

        // 置き換え（サイズの差だけ増える）、新規、フォルダマーカー（件数に数えない）
        upsert(
            &db,
            "c",
            vec![object("a/x.txt", 30), object("a/z.txt", 1), object("d/", 0)],
        )
        .await
        .unwrap();
        assert_eq!(counts(&db).await, (3, 36));

        // インデックスにないキーの削除は数えない
        remove(
            &db,
            "c",
            vec!["a/y.txt".into(), "b/".into(), "missing.txt".into()],
        )
        .await
        .unwrap();
        assert_eq!(counts(&db).await, (2, 31));

        // 差分で更新した値は、数え直した値と一致する
        db.run(|c| {
            let tx = c.transaction()?;
            refresh_counts(&tx, "c")?;
            tx.commit()?;
            Ok(())
        })
        .await
        .unwrap();
        assert_eq!(counts(&db).await, (2, 31));
    }

    #[tokio::test]
    async fn limits_folder_queries_to_the_keys_under_the_prefix() {
        let (db, _dir) = Db::open_temp().unwrap();
        let at = "2026-01-01T00:00:00Z";
        seed(
            &db,
            "c",
            &[
                ("a/x.txt", 10, at, StorageClass::Standard),
                ("a/b/y.txt", 5, at, StorageClass::Standard),
                // 「a/」で始まらない、並び順で前後のキー
                ("a.txt", 100, at, StorageClass::Standard),
                ("a0.txt", 100, at, StorageClass::Standard),
                ("ab/z.txt", 100, at, StorageClass::Standard),
            ],
        )
        .await;
        let a = folder_summary(&db, "c", "a/").await.unwrap().unwrap();
        assert_eq!((a.item_count, a.total_bytes), (2, 15));
        let root = folder_summary(&db, "c", "").await.unwrap().unwrap();
        assert_eq!(root.total_bytes, 315);

        // 配下がなくなったフォルダだけを取り除く（「a/」には x.txt が残る）
        remove(&db, "c", vec!["a/b/y.txt".into()]).await.unwrap();
        let prefixes: Vec<String> = db
            .run(|c| {
                let mut stmt = c.prepare(
                    "SELECT prefix FROM prefixes WHERE connection_id = 'c' ORDER BY prefix",
                )?;
                let rows = stmt
                    .query_map([], |r| r.get(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await
            .unwrap();
        assert_eq!(prefixes, ["a/", "ab/"]);
    }

    #[tokio::test]
    async fn ignores_an_index_whose_first_scan_never_finished() {
        let (db, _dir) = Db::open_temp().unwrap();
        // 初回の走査が失敗・キャンセルで終わると、途中までの行と、走査日時のない状態が残る
        db.run(|c| {
            let tx = c.transaction()?;
            tx.execute(
                "INSERT INTO index_state (connection_id, status, generation) VALUES ('c', 'ready', 1)",
                [],
            )?;
            write_object(&tx, "c", &object("a/x.txt", 10), 1)?;
            tx.commit()?;
            Ok(())
        })
        .await
        .unwrap();
        // 一部しかないため、集計には使わない（S3 の列挙・CloudWatch に任せる）
        assert!(folder_summary(&db, "c", "a/").await.unwrap().is_none());
        assert!(class_totals(&db, "c").await.unwrap().is_none());
        // 自動更新が無効（0）でも、古いものとして次の検索で走査し直す
        let s = status(&db, "c", 0, None).await.unwrap();
        assert_eq!((s.state, s.last_scan_at), (IndexState::Stale, None));
    }

    #[tokio::test]
    async fn returns_folder_marker_dates() {
        let (db, _dir) = Db::open_temp().unwrap();
        seed(
            &db,
            "c",
            &[
                ("p/sub/", 0, "2026-09-27T05:32:00Z", StorageClass::Standard),
                (
                    "p/file.txt",
                    5,
                    "2026-09-26T00:00:00Z",
                    StorageClass::Standard,
                ),
            ],
        )
        .await;
        let dates = folder_marker_dates(&db, "c", "p/").await.unwrap();
        assert_eq!(
            dates.get("p/sub/").map(String::as_str),
            Some("2026-09-27T05:32:00Z")
        );
        assert!(!dates.contains_key("p/file.txt"));
    }

    #[tokio::test]
    async fn reports_status_and_staleness() {
        let (db, _dir) = Db::open_temp().unwrap();
        assert_eq!(
            status(&db, "c", 60, None).await.unwrap().state,
            IndexState::None
        );
        seed(
            &db,
            "c",
            &[("a.txt", 3, "2026-01-01T00:00:00Z", StorageClass::Glacier)],
        )
        .await;
        let s = status(&db, "c", 60, None).await.unwrap();
        assert_eq!((s.state, s.object_count), (IndexState::Ready, 1));
        assert_eq!(
            status(&db, "c", 60, Some(10)).await.unwrap().state,
            IndexState::Building
        );
        let (totals, count) = class_totals(&db, "c").await.unwrap().unwrap();
        assert_eq!(totals[&StorageClass::Glacier], 3);
        assert_eq!(count, 1);
        delete_index(&db, "c").await.unwrap();
        assert_eq!(
            status(&db, "c", 60, None).await.unwrap().state,
            IndexState::None
        );
    }
}

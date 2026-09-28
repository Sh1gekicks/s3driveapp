//! アップロード（04 §4）。「準備」で計画を作り、同名の項目の扱いを決めてから「開始」する。

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use aws_sdk_s3::types::{ChecksumAlgorithm, CompletedMultipartUpload, CompletedPart, EncodingType};
use aws_smithy_types::byte_stream::{ByteStream, Length};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures::{StreamExt, stream};
use rusqlite::params;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use super::body::counting_stream;
use super::multipart::{self, IN_MEMORY_PART_LIMIT};
use super::{FileWork, JobState, with_retry};
use crate::Core;
use crate::aws::error_map::{self, Ctx};
use crate::connections::ConnCtx;
use crate::error::{CoreError, CoreResult, ErrorCode};
use crate::model::{
    ConflictDecision, Decisions, ExcludeReason, ExcludedItem, JobId, StorageClass, TransferKind,
    UploadConflict, UploadPlan,
};
use crate::search::{self, IndexedObject};
use crate::store::Db;
use crate::util::{key, time};

/// アップロードする 1 件（フォルダマーカーを含む）。
#[derive(Debug, Clone)]
pub struct UploadFile {
    pub path: PathBuf,
    pub key: String,
    pub size: u64,
    pub mtime: Option<SystemTime>,
    pub storage_class: StorageClass,
    /// 既存の項目を置き換える（作成日を引き継ぐ。04 §4.3）。
    pub replaces: bool,
    /// 空のフォルダ（フォルダマーカーとして作る）。
    pub is_marker: bool,
}

impl UploadFile {
    pub(crate) fn display_name(&self) -> String {
        key::nfc(key::base_name(&self.key))
    }
}

/// `upload_prepare` で作った計画（`upload_start` まで保持する）。
#[derive(Debug, Clone)]
pub struct UploadPlanData {
    pub connection_id: String,
    pub prefix: String,
    pub files: Vec<UploadFile>,
    /// アップロード先の各フォルダにある名前（「両方を残す」の連番に使う）。
    pub existing_names: HashMap<String, HashSet<String>>,
    pub conflicts: HashSet<String>,
}

/// 展開したローカルの項目（パス、キー、サイズ、更新日時、空のフォルダか）。
pub type LocalItem = (PathBuf, String, u64, Option<SystemTime>, bool);

/// ローカルの選択を展開する（シンボリックリンクと除外対象を除く）。
pub fn expand_selection(
    paths: &[PathBuf],
    prefix: &str,
    ignore: &[String],
    normalize_nfc: bool,
) -> (Vec<LocalItem>, Vec<ExcludedItem>) {
    let mut files = Vec::new();
    let mut excluded = Vec::new();
    let to_key = |rel: &Path, dir: bool| -> Result<String, ExcludeReason> {
        let parts: Vec<String> = rel
            .components()
            .map(|c| {
                let s = c.as_os_str().to_string_lossy().into_owned();
                if normalize_nfc { key::nfc(&s) } else { s }
            })
            .collect();
        let mut k = format!("{prefix}{}", parts.join("/"));
        if dir {
            k.push('/');
        }
        if key::has_control_chars(&k) {
            return Err(ExcludeReason::InvalidChar);
        }
        if k.len() > key::MAX_KEY_BYTES {
            return Err(ExcludeReason::KeyTooLong);
        }
        Ok(k)
    };
    let display = |p: &Path| {
        key::nfc(
            &p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        )
    };

    for root in paths {
        let meta = match std::fs::symlink_metadata(root) {
            Ok(m) => m,
            Err(_) => {
                excluded.push(ExcludedItem {
                    name: display(root),
                    reason: ExcludeReason::Unreadable,
                });
                continue;
            }
        };
        let base = root.parent().unwrap_or(Path::new(""));
        if meta.file_type().is_symlink() {
            excluded.push(ExcludedItem {
                name: display(root),
                reason: ExcludeReason::Symlink,
            });
            continue;
        }
        if meta.is_file() {
            if ignore.iter().any(|i| i == &display(root)) {
                excluded.push(ExcludedItem {
                    name: display(root),
                    reason: ExcludeReason::Ignored,
                });
                continue;
            }
            match to_key(root.strip_prefix(base).unwrap_or(root), false) {
                Ok(k) => files.push((root.clone(), k, meta.len(), meta.modified().ok(), false)),
                Err(reason) => excluded.push(ExcludedItem {
                    name: display(root),
                    reason,
                }),
            }
            continue;
        }
        for entry in walkdir::WalkDir::new(root)
            .follow_links(false)
            .sort_by_file_name()
        {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    let name = e.path().map(display).unwrap_or_default();
                    excluded.push(ExcludedItem {
                        name,
                        reason: ExcludeReason::Unreadable,
                    });
                    continue;
                }
            };
            let path = entry.path();
            let name = display(path);
            let rel = path.strip_prefix(base).unwrap_or(path);
            if entry.path_is_symlink() {
                excluded.push(ExcludedItem {
                    name,
                    reason: ExcludeReason::Symlink,
                });
                continue;
            }
            if ignore.iter().any(|i| i == &name) {
                if entry.file_type().is_file() {
                    excluded.push(ExcludedItem {
                        name,
                        reason: ExcludeReason::Ignored,
                    });
                }
                continue;
            }
            if entry.file_type().is_dir() {
                let empty = std::fs::read_dir(path)
                    .map(|mut d| d.next().is_none())
                    .unwrap_or(false);
                if empty {
                    match to_key(rel, true) {
                        Ok(k) => files.push((path.to_path_buf(), k, 0, None, true)),
                        Err(reason) => excluded.push(ExcludedItem { name, reason }),
                    }
                }
                continue;
            }
            match entry.metadata() {
                Ok(m) => match to_key(rel, false) {
                    Ok(k) => files.push((path.to_path_buf(), k, m.len(), m.modified().ok(), false)),
                    Err(reason) => excluded.push(ExcludedItem { name, reason }),
                },
                Err(_) => excluded.push(ExcludedItem {
                    name,
                    reason: ExcludeReason::Unreadable,
                }),
            }
        }
    }
    (files, excluded)
}

/// アップロード先のフォルダの中身（キー → サイズ・更新日時）と名前の一覧。
async fn list_folder(
    ctx: &ConnCtx,
    prefix: &str,
) -> CoreResult<(BTreeMap<String, (u64, String)>, HashSet<String>)> {
    let mut files = BTreeMap::new();
    let mut names = HashSet::new();
    let mut token = None;
    loop {
        let out = ctx
            .clients
            .s3
            .list_objects_v2()
            .bucket(&ctx.bucket)
            .prefix(prefix)
            .delimiter("/")
            .encoding_type(EncodingType::Url)
            .set_continuation_token(token)
            .send()
            .await
            .map_err(|e| error_map::classify(&e, Ctx::Op("s3:ListBucket")))?;
        for o in out.contents() {
            let Some(k) = o.key().map(key::url_decode_key) else {
                continue;
            };
            names.insert(key::base_name(&k).to_string());
            files.insert(
                k,
                (
                    o.size().unwrap_or(0).max(0) as u64,
                    o.last_modified()
                        .map(time::aws_to_rfc3339)
                        .unwrap_or_default(),
                ),
            );
        }
        for p in out.common_prefixes().iter().filter_map(|p| p.prefix()) {
            names.insert(key::base_name(&key::url_decode_key(p)).to_string());
        }
        token = out.next_continuation_token().map(str::to_string);
        if token.is_none() {
            return Ok((files, names));
        }
    }
}

impl Core {
    /// アップロードの準備（04 §4.1）。同名の項目・除外した項目を含む計画を返す。
    pub async fn upload_prepare(
        &self,
        connection_id: &str,
        prefix: &str,
        selection_id: &str,
    ) -> CoreResult<UploadPlan> {
        key::validate_prefix(prefix)?;
        let ctx = self.ctx(connection_id).await?;
        let paths = self.0.selections.get(selection_id)?;
        let settings = self.0.settings.settings().transfer;
        let class = ctx
            .record
            .default_storage_class
            .unwrap_or(settings.default_storage_class);
        let (prefix_owned, ignore, nfc) = (
            prefix.to_string(),
            settings.ignore.clone(),
            settings.normalize_nfc,
        );
        let (expanded, excluded) = tokio::task::spawn_blocking(move || {
            expand_selection(&paths, &prefix_owned, &ignore, nfc)
        })
        .await?;

        // アップロード先の各フォルダを列挙して同名の項目を探す
        let parents: HashSet<String> = expanded
            .iter()
            .map(|f| key::parent_prefix(&f.1).to_string())
            .collect();
        let mut existing = BTreeMap::new();
        let mut existing_names = HashMap::new();
        for parent in parents {
            let (files, names) = list_folder(&ctx, &parent).await?;
            existing.extend(files);
            existing_names.insert(parent, names);
        }

        let mut conflicts = Vec::new();
        let mut conflict_keys = HashSet::new();
        let files: Vec<UploadFile> = expanded
            .into_iter()
            .map(|(path, k, size, mtime, is_marker)| {
                if !is_marker && let Some((remote_size, remote_modified)) = existing.get(&k) {
                    conflict_keys.insert(k.clone());
                    conflicts.push(UploadConflict {
                        key: k.clone(),
                        local_size: size,
                        remote_size: *remote_size,
                        remote_modified: remote_modified.clone(),
                    });
                }
                UploadFile {
                    path,
                    key: k,
                    size,
                    mtime,
                    storage_class: class,
                    replaces: false,
                    is_marker,
                }
            })
            .collect();
        let plan_id = uuid::Uuid::new_v4().to_string();
        let plan = UploadPlan {
            plan_id: plan_id.clone(),
            file_count: files.iter().filter(|f| !f.is_marker).count() as u64,
            total_bytes: files.iter().map(|f| f.size).sum(),
            conflicts,
            excluded,
            versioning_enabled: ctx.versioning().await.has_history(),
        };
        self.0.transfers.store_plan(
            plan_id,
            UploadPlanData {
                connection_id: connection_id.to_string(),
                prefix: prefix.to_string(),
                files,
                existing_names,
                conflicts: conflict_keys,
            },
        );
        Ok(plan)
    }

    /// アップロードの開始。同名の項目は `decisions` に従う（置き換え／スキップ／両方を残す）。
    pub async fn upload_start(&self, plan_id: &str, decisions: Decisions) -> CoreResult<JobId> {
        let mut plan = self.0.transfers.take_plan(plan_id)?;
        let ctx = self.ctx(&plan.connection_id).await?;
        let mut files = Vec::new();
        for mut f in std::mem::take(&mut plan.files) {
            if plan.conflicts.contains(&f.key) {
                match decisions.get(&f.key) {
                    ConflictDecision::Skip => continue,
                    ConflictDecision::Replace => f.replaces = true,
                    ConflictDecision::KeepBoth => {
                        let parent = key::parent_prefix(&f.key).to_string();
                        let names = plan.existing_names.entry(parent.clone()).or_default();
                        let unique =
                            key::unique_name(key::base_name(&f.key), |n| names.contains(n));
                        names.insert(unique.clone());
                        f.key = format!("{parent}{unique}");
                    }
                }
            }
            files.push(FileWork::Upload(f));
        }
        let count = files
            .iter()
            .filter(|f| matches!(f, FileWork::Upload(u) if !u.is_marker))
            .count();
        let options = self.0.settings.settings().transfer.clamped();
        self.0.transfers.set_max_files(options.max_files);
        let destination = format!("{}/{}", ctx.bucket, plan.prefix);
        Ok(self.0.transfers.enqueue(
            ctx,
            TransferKind::Upload,
            format!("{count} 件をアップロード中"),
            destination,
            options,
            files,
        ))
    }

    /// 失敗したファイルだけを再実行する（`transfer_retry`）。
    pub fn transfer_retry(&self, job_id: &str) -> CoreResult<JobId> {
        let (job, files) = self.0.transfers.retry_files(job_id)?;
        if files.is_empty() {
            return Err(CoreError::with_message(
                ErrorCode::NotFound,
                "再試行する項目がありません",
            ));
        }
        let snapshot = self
            .0
            .transfers
            .job_snapshot(job_id)
            .ok_or_else(|| CoreError::new(ErrorCode::NotFound))?;
        Ok(self.0.transfers.enqueue(
            job.ctx.clone(),
            snapshot.kind,
            snapshot.title,
            snapshot.destination,
            job.options.clone(),
            files,
        ))
    }

    /// アプリが中断したマルチパートアップロードの後始末（04 §14.5）。サインイン後に呼ぶ。
    pub async fn abort_stale_uploads(&self) -> CoreResult<usize> {
        let rows: Vec<(String, String, String, String)> = self
            .0
            .db
            .run(|c| {
                let mut stmt = c.prepare(
                    "SELECT id, connection_id, key, upload_id FROM transfers WHERE direction = 'upload' AND upload_id IS NOT NULL",
                )?;
                let rows = stmt
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await?;
        let mut aborted = 0;
        for (id, cid, k, upload_id) in rows {
            let Ok(ctx) = self.ctx(&cid).await else {
                continue;
            };
            let result = ctx
                .clients
                .s3
                .abort_multipart_upload()
                .bucket(&ctx.bucket)
                .key(&k)
                .upload_id(&upload_id)
                .send()
                .await;
            let gone = abort_settled(
                &result
                    .map(|_| ())
                    .map_err(|e| error_map::classify(&e, Ctx::Op("s3:AbortMultipartUpload"))),
            );
            if gone {
                aborted += 1;
                forget_upload(&self.0.db, &id).await?;
            }
        }
        Ok(aborted)
    }
}

async fn forget_upload(db: &Db, id: &str) -> CoreResult<()> {
    let id = id.to_string();
    db.run(move |c| {
        c.execute("DELETE FROM transfers WHERE id = ?1", [id])?;
        Ok(())
    })
    .await
}

fn rfc3339(t: SystemTime) -> String {
    time::to_rfc3339(DateTime::<Utc>::from(t))
}

/// ファイルが送信中に変更されていないか（サイズと更新日時）。
fn ensure_unchanged(file: &UploadFile) -> CoreResult<()> {
    let meta = std::fs::metadata(&file.path).map_err(|e| CoreError::local_io(&file.path, &e))?;
    if meta.len() != file.size || meta.modified().ok() != file.mtime {
        return Err(CoreError::new(ErrorCode::FileChanged));
    }
    Ok(())
}

/// 1 件をアップロードする。
pub(super) async fn run(
    db: &Db,
    job: &JobState,
    file: &UploadFile,
    counter: Arc<AtomicU64>,
) -> CoreResult<()> {
    let ctx = &job.ctx;
    let s3 = &ctx.clients.s3_transfer;
    if file.is_marker {
        s3.put_object()
            .bucket(&ctx.bucket)
            .key(&file.key)
            .content_length(0)
            .send()
            .await
            .map_err(|e| error_map::classify(&e, Ctx::Op("s3:PutObject")))?;
        return index(db, &ctx.id, file, None).await;
    }

    // 作成日: 新規はアップロード日時、置き換えは既存の作成日を引き継ぐ（04 §4.3）
    let mut created = time::now_rfc3339();
    if file.replaces
        && let Ok(existing) = crate::objects::head(ctx, &file.key, None).await
    {
        created = existing
            .metadata
            .get("s3drive-created")
            .cloned()
            .unwrap_or(existing.last_modified);
    }
    let mut metadata = HashMap::new();
    metadata.insert("s3drive-created".to_string(), created);
    if let Some(m) = file.mtime {
        metadata.insert("s3drive-mtime".to_string(), rfc3339(m));
    }
    let content_type = mime_guess::from_path(&file.key)
        .first_or_octet_stream()
        .essence_str()
        .to_string();
    let threshold = job.options.multipart_threshold_mb as u64 * multipart::MIB;

    let etag = if file.size < threshold {
        let data = tokio::fs::read(&file.path)
            .await
            .map_err(|e| CoreError::local_io(&file.path, &e))?;
        if data.len() as u64 != file.size {
            return Err(CoreError::new(ErrorCode::FileChanged));
        }
        let data = Bytes::from(data);
        let send = s3
            .put_object()
            .bucket(&ctx.bucket)
            .key(&file.key)
            .content_type(&content_type)
            .content_length(file.size as i64)
            .set_metadata(Some(metadata))
            .storage_class(file.storage_class.to_sdk())
            .body(counting_stream(data, counter).0)
            .send();
        let out = tokio::select! {
            r = send => r.map_err(|e| error_map::classify(&e, Ctx::Op("s3:PutObject")))?,
            _ = job.cancel.cancelled() => return Err(CoreError::canceled()),
        };
        crate::objects::trim_etag(out.e_tag())
    } else {
        multipart_upload(db, job, file, &content_type, metadata, counter).await?
    };
    index(db, &ctx.id, file, Some(etag)).await
}

async fn index(
    db: &Db,
    connection_id: &str,
    file: &UploadFile,
    etag: Option<String>,
) -> CoreResult<()> {
    search::upsert(
        db,
        connection_id,
        vec![IndexedObject {
            key: file.key.clone(),
            size: file.size,
            last_modified: time::now_rfc3339(),
            etag,
            storage_class: if file.is_marker {
                StorageClass::Standard
            } else {
                file.storage_class
            },
        }],
    )
    .await
}

async fn multipart_upload(
    db: &Db,
    job: &JobState,
    file: &UploadFile,
    content_type: &str,
    metadata: HashMap<String, String>,
    counter: Arc<AtomicU64>,
) -> CoreResult<String> {
    let ctx = &job.ctx;
    let s3 = &ctx.clients.s3_transfer;
    let mut create = s3
        .create_multipart_upload()
        .bucket(&ctx.bucket)
        .key(&file.key)
        .content_type(content_type)
        .set_metadata(Some(metadata))
        .storage_class(file.storage_class.to_sdk());
    if ctx.clients.full_checksums {
        create = create.checksum_algorithm(ChecksumAlgorithm::Crc32);
    }
    let created = create
        .send()
        .await
        .map_err(|e| error_map::classify(&e, Ctx::Op("s3:PutObject")))?;
    let upload_id = created
        .upload_id()
        .ok_or_else(|| CoreError::internal("no upload id"))?
        .to_string();
    let part_size = multipart::part_size(file.size);

    // 完了まで SQLite に記録し、アプリが中断した場合に起動時に中止できるようにする（04 §14.5）
    let transfer_id = uuid::Uuid::new_v4().to_string();
    {
        let (tid, jid, cid, k, path, uid) = (
            transfer_id.clone(),
            job.id.clone(),
            ctx.id.clone(),
            file.key.clone(),
            file.path.display().to_string(),
            upload_id.clone(),
        );
        let size = file.size as i64;
        db.run(move |c| {
            c.execute(
                "INSERT INTO transfers (id, job_id, connection_id, direction, key, local_path, size, status, upload_id, part_size)
                 VALUES (?1, ?2, ?3, 'upload', ?4, ?5, ?6, 'running', ?7, ?8)",
                params![tid, jid, cid, k, path, size, uid, part_size as i64],
            )?;
            Ok(())
        })
        .await?;
    }

    let result = upload_parts_and_complete(job, file, &upload_id, part_size, counter).await;
    let finished = match &result {
        Ok(_) => true,
        // キャンセル・失敗時は未完了のマルチパートアップロードを中止する
        Err(_) => {
            let aborted = s3
                .abort_multipart_upload()
                .bucket(&ctx.bucket)
                .key(&file.key)
                .upload_id(&upload_id)
                .send()
                .await
                .map(|_| ())
                .map_err(|e| error_map::classify(&e, Ctx::Op("s3:AbortMultipartUpload")));
            abort_settled(&aborted)
        }
    };
    // 中止できなかった場合（通信断など）は記録を残し、次回の起動時に中止し直す（04 §14.5）
    if finished {
        forget_upload(db, &transfer_id).await?;
    }
    result
}

/// `AbortMultipartUpload` の結果から、アップロードの後始末が済んだか（記録を消してよいか）を判定する。
/// すでにない（`NoSuchUpload`）場合も済んだものとして扱う。
fn abort_settled(result: &CoreResult<()>) -> bool {
    match result {
        Ok(()) => true,
        Err(e) => e.code == ErrorCode::NotFound,
    }
}

async fn upload_parts_and_complete(
    job: &JobState,
    file: &UploadFile,
    upload_id: &str,
    part_size: u64,
    counter: Arc<AtomicU64>,
) -> CoreResult<String> {
    let ctx = &job.ctx;
    let s3 = &ctx.clients.s3_transfer;
    let ranges = multipart::part_ranges(file.size, part_size);
    let concurrency = job.options.max_parts_per_file.max(1) as usize;
    let results: Vec<CoreResult<CompletedPart>> = stream::iter(ranges.into_iter().enumerate())
        .map(|(i, (offset, len))| {
            let counter = counter.clone();
            async move {
                let part_number = (i + 1) as i32;
                with_retry(&job.cancel, || {
                    let counter = counter.clone();
                    async move {
                        upload_part(job, file, upload_id, part_number, offset, len, counter).await
                    }
                })
                .await
            }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;
    let mut parts = results.into_iter().collect::<CoreResult<Vec<_>>>()?;
    parts.sort_by_key(|p| p.part_number());
    ensure_unchanged(file)?;
    let complete = s3
        .complete_multipart_upload()
        .bucket(&ctx.bucket)
        .key(&file.key)
        .upload_id(upload_id)
        .multipart_upload(
            CompletedMultipartUpload::builder()
                .set_parts(Some(parts))
                .build(),
        )
        .send();
    let out = tokio::select! {
        r = complete => r.map_err(|e| error_map::classify(&e, Ctx::Op("s3:PutObject")))?,
        _ = job.cancel.cancelled() => return Err(CoreError::canceled()),
    };
    Ok(crate::objects::trim_etag(out.e_tag()))
}

async fn upload_part(
    job: &JobState,
    file: &UploadFile,
    upload_id: &str,
    part_number: i32,
    offset: u64,
    len: u64,
    file_counter: Arc<AtomicU64>,
) -> CoreResult<CompletedPart> {
    let ctx = &job.ctx;
    // メモリに読み込めるパートは送信しながら数える。大きなパートは完了時にまとめて加える
    let (body, counted) = if len <= IN_MEMORY_PART_LIMIT {
        let mut f = tokio::fs::File::open(&file.path)
            .await
            .map_err(|e| CoreError::local_io(&file.path, &e))?;
        f.seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(|e| CoreError::local_io(&file.path, &e))?;
        let mut buf = vec![0u8; len as usize];
        f.read_exact(&mut buf).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                CoreError::new(ErrorCode::FileChanged)
            } else {
                CoreError::local_io(&file.path, &e)
            }
        })?;
        let (stream, sent) = counting_stream(Bytes::from(buf), file_counter.clone());
        (stream, Some(sent))
    } else {
        let stream = ByteStream::read_from()
            .path(&file.path)
            .offset(offset)
            .length(Length::Exact(len))
            .build()
            .await
            .map_err(|e| CoreError::local_io(&file.path, &std::io::Error::other(e.to_string())))?;
        (stream, None)
    };
    let mut req = ctx
        .clients
        .s3_transfer
        .upload_part()
        .bucket(&ctx.bucket)
        .key(&file.key)
        .upload_id(upload_id)
        .part_number(part_number)
        .content_length(len as i64)
        .body(body);
    if ctx.clients.full_checksums {
        req = req.checksum_algorithm(ChecksumAlgorithm::Crc32);
    }
    match req.send().await {
        Ok(out) => {
            if counted.is_none() {
                file_counter.fetch_add(len, Ordering::Relaxed);
            }
            Ok(CompletedPart::builder()
                .part_number(part_number)
                .set_e_tag(out.e_tag().map(str::to_string))
                .set_checksum_crc32(out.checksum_crc32().map(str::to_string))
                .build())
        }
        Err(e) => {
            // 失敗した試行で数えた分を戻す（再試行で数え直す）
            if let Some(sent) = counted {
                let n = sent.swap(0, Ordering::Relaxed);
                file_counter.fetch_sub(
                    n.min(file_counter.load(Ordering::Relaxed)),
                    Ordering::Relaxed,
                );
            }
            Err(error_map::classify(&e, Ctx::Op("s3:PutObject")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_record_when_the_abort_fails() {
        assert!(abort_settled(&Ok(())));
        // すでに中止・完了済み（NoSuchUpload）
        assert!(abort_settled(&Err(CoreError::new(ErrorCode::NotFound))));
        // 通信断などで中止できなかった場合は、次回の起動時に中止し直す（04 §14.5）
        assert!(!abort_settled(&Err(CoreError::new(ErrorCode::Network))));
        assert!(!abort_settled(&Err(CoreError::new(
            ErrorCode::AccessDenied
        ))));
    }
}

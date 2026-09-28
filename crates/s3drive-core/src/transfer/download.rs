//! ダウンロード（04 §5）。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use aws_sdk_s3::types::ChecksumMode;
use futures::{StreamExt, stream};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

use super::multipart::{DOWNLOAD_RANGE_SIZE, RANGED_DOWNLOAD_THRESHOLD, part_ranges};
use super::{FileWork, JobState, with_retry};
use crate::Core;
use crate::aws::error_map::{self, Ctx};
use crate::error::{CoreError, CoreResult, ErrorCode};
use crate::model::{DownloadDestination, JobId, Target, TransferKind};
use crate::objects::{head, list_recursive};
use crate::util::{key, time};

const TEMP_SUFFIX: &str = ".s3drive-download";

/// ダウンロードする 1 件（フォルダマーカーは空のフォルダとして作る）。
#[derive(Debug, Clone)]
pub struct DownloadFile {
    pub key: String,
    pub version_id: Option<String>,
    pub size: u64,
    pub etag: String,
    /// 保存先のフォルダ。
    pub dir: PathBuf,
    /// 保存する名前（同名のファイルがあれば実行時に連番を付ける）。
    pub file_name: String,
    pub is_marker: bool,
    /// 保存先の外を指す名前（`..` など）のため保存しない。実行すると `INVALID_NAME` で失敗にする。
    pub rejected: bool,
}

impl DownloadFile {
    pub(crate) fn display_name(&self) -> String {
        self.file_name.clone()
    }

    /// 保存先の外を指す名前のため保存しない項目（失敗の一覧に表示する）。
    fn rejected(key: &str, version_id: Option<String>) -> Self {
        Self {
            key: key.to_string(),
            version_id,
            size: 0,
            etag: String::new(),
            dir: PathBuf::new(),
            file_name: key.to_string(),
            is_marker: false,
            rejected: true,
        }
    }
}

fn unsafe_name_error(key: &str) -> CoreError {
    CoreError::with_message(
        ErrorCode::InvalidName,
        "保存先の外を指す名前のため、ダウンロードしませんでした",
    )
    .detail(key.to_string())
}

/// 既定のダウンロード先（`~/Downloads`）。
pub fn default_download_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Downloads")
}

/// 表示用のパス（ホームを `~` にする）。
pub fn display_path(path: &Path) -> String {
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from)
        && let Ok(rest) = path.strip_prefix(&home)
    {
        return format!("~/{}", rest.display());
    }
    path.display().to_string()
}

#[cfg(unix)]
fn available_space(path: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: c は NUL 終端のパス、stat は書き込み可能な領域
    let rc = unsafe { libc::statvfs(c.as_ptr(), &mut stat) };
    (rc == 0).then(|| stat.f_bavail as u64 * stat.f_frsize as u64)
}

#[cfg(not(unix))]
fn available_space(_path: &Path) -> Option<u64> {
    None
}

/// バージョンを指定したダウンロードの名前（「{名前} ({YYYY-MM-DD HH.mm}){拡張子}」。04 §5.1）。
pub fn version_file_name(name: &str, last_modified: &str) -> String {
    let stamp = time::parse_rfc3339(last_modified)
        .map(time::version_file_stamp)
        .unwrap_or_else(|| "version".to_string());
    let (stem, ext) = key::split_ext(name);
    format!("{stem} ({stamp}){ext}")
}

impl Core {
    /// ダウンロードの開始（04 §5.2）。取り出していないアーカイブは `INVALID_OBJECT_STATE`（DLG-07 を表示する）。
    pub async fn download_start(
        &self,
        connection_id: &str,
        targets: Vec<Target>,
        destination: DownloadDestination,
    ) -> CoreResult<JobId> {
        let ctx = self.ctx(connection_id).await?;
        let settings = self.0.settings.settings();
        let dir = match destination {
            DownloadDestination::Keyword(_) => settings
                .general
                .download_dir
                .map(PathBuf::from)
                .unwrap_or_else(default_download_dir),
            DownloadDestination::Selection { selection_id } => self
                .0
                .selections
                .get(&selection_id)?
                .into_iter()
                .next()
                .ok_or_else(|| CoreError::new(ErrorCode::NotFound))?,
        };
        let mut files = Vec::new();
        for t in &targets {
            key::validate_key(&t.key)?;
            if t.is_folder {
                let folder_name = key::nfc(key::base_name(&t.key));
                let (objects, _) = list_recursive(&ctx, &t.key, usize::MAX).await?;
                for o in objects {
                    let rest = o.key.strip_prefix(t.key.as_str()).unwrap_or(&o.key);
                    // キーの階層をそのまま連結すると `..` で保存先の外に書き込めるため、検査してから使う（05 §3.9）
                    let segments = key::is_safe_local_name(&folder_name)
                        .then(|| key::local_segments(rest))
                        .flatten();
                    let Some(segments) = segments else {
                        files.push(DownloadFile::rejected(&o.key, None));
                        continue;
                    };
                    let rel: PathBuf = std::iter::once(folder_name.clone())
                        .chain(segments)
                        .collect();
                    let is_marker = key::is_folder_key(&o.key);
                    let (sub_dir, name) = if is_marker {
                        (dir.join(&rel), String::new())
                    } else {
                        (
                            dir.join(rel.parent().unwrap_or(Path::new(""))),
                            rel.file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                        )
                    };
                    files.push(DownloadFile {
                        key: o.key.clone(),
                        version_id: None,
                        size: if is_marker { 0 } else { o.size },
                        etag: o.etag.clone(),
                        dir: sub_dir,
                        file_name: name,
                        is_marker,
                        rejected: false,
                    });
                }
            } else {
                let name = key::nfc(key::base_name(&t.key));
                if !key::is_safe_local_name(&name) {
                    files.push(DownloadFile::rejected(&t.key, t.version_id.clone()));
                    continue;
                }
                let info = head(&ctx, &t.key, t.version_id.as_deref()).await?;
                if info.restore.needs_restore() {
                    return Err(CoreError::new(ErrorCode::InvalidObjectState).detail(t.key.clone()));
                }
                files.push(DownloadFile {
                    key: t.key.clone(),
                    version_id: t.version_id.clone(),
                    size: info.size,
                    etag: info.etag.clone(),
                    dir: dir.clone(),
                    file_name: if t.version_id.is_some() {
                        version_file_name(&name, &info.last_modified)
                    } else {
                        name
                    },
                    is_marker: false,
                    rejected: false,
                });
            }
        }
        // 開始前に空き容量を確認する（04 §5.4）
        let total: u64 = files.iter().map(|f| f.size).sum();
        std::fs::create_dir_all(&dir).map_err(|e| {
            CoreError::with_message(ErrorCode::LocalIo, "保存先に書き込めません")
                .detail(e.to_string())
        })?;
        if let Some(free) = available_space(&dir)
            && free < total
        {
            return Err(CoreError::new(ErrorCode::DiskFull));
        }
        let title = match files.iter().filter(|f| !f.is_marker).count() {
            1 if targets.len() == 1 && !targets[0].is_folder => {
                format!("「{}」をダウンロード中", files[0].file_name)
            }
            _ if targets.len() == 1 => format!(
                "「{}」をダウンロード中",
                key::nfc(key::base_name(&targets[0].key))
            ),
            n => format!("{n} 件をダウンロード中"),
        };
        let options = settings.transfer.clamped();
        self.0.transfers.set_max_files(options.max_files);
        Ok(self.0.transfers.enqueue(
            ctx,
            TransferKind::Download,
            title,
            display_path(&dir),
            options,
            files.into_iter().map(FileWork::Download).collect(),
        ))
    }
}

/// 同名のファイルがあれば「name (1).ext」のように連番を付けた保存先を予約する（04 §5.1）。
fn reserve_path(job: &JobState, dir: &Path, name: &str) -> PathBuf {
    let mut reserved = job.reserved_paths.lock().unwrap();
    let taken = |n: &str| {
        let p = dir.join(n);
        p.exists() || reserved.contains(&p) || dir.join(format!("{n}{TEMP_SUFFIX}")).exists()
    };
    let unique = key::unique_name(name, taken);
    let path = dir.join(unique);
    reserved.insert(path.clone());
    path
}

/// 1 件をダウンロードする。
pub(super) async fn run(
    job: &JobState,
    file: &DownloadFile,
    counter: Arc<AtomicU64>,
) -> CoreResult<()> {
    if file.rejected {
        return Err(unsafe_name_error(&file.key));
    }
    if file.is_marker {
        return tokio::fs::create_dir_all(&file.dir)
            .await
            .map_err(|e| CoreError::local_io(&file.dir, &e));
    }
    tokio::fs::create_dir_all(&file.dir).await.map_err(|e| {
        CoreError::with_message(ErrorCode::LocalIo, "保存先に書き込めません").detail(e.to_string())
    })?;
    let target = reserve_path(job, &file.dir, &file.file_name);
    let temp = PathBuf::from(format!("{}{TEMP_SUFFIX}", target.display()));

    let mut result = download_to(job, file, &temp, counter.clone()).await;
    if matches!(&result, Err(e) if e.code == ErrorCode::PreconditionFailed) {
        // ダウンロード中に置き換わった: 最初から 1 回だけやり直す（04 §5.4）
        counter.store(0, Ordering::Relaxed);
        let refreshed = head(&job.ctx, &file.key, file.version_id.as_deref()).await?;
        let retry = DownloadFile {
            etag: refreshed.etag,
            size: refreshed.size,
            ..file.clone()
        };
        result = download_to(job, &retry, &temp, counter).await;
    }
    let finished = async {
        let mtime = result?;
        tokio::fs::rename(&temp, &target)
            .await
            .map_err(|e| CoreError::local_io(&target, &e))?;
        if let Some(mtime) = mtime {
            let ft = filetime::FileTime::from_unix_time(
                mtime.timestamp(),
                mtime.timestamp_subsec_nanos(),
            );
            if let Err(e) = filetime::set_file_mtime(&target, ft) {
                log::debug!("更新日時を設定できません: {e}");
            }
        }
        job.saved.lock().unwrap().push(target.clone());
        Ok(())
    }
    .await;
    if finished.is_err() {
        let _ = tokio::fs::remove_file(&temp).await;
        job.reserved_paths.lock().unwrap().remove(&target);
    }
    finished
}

/// 一時ファイルに書き込み、復元する更新日時を返す。
async fn download_to(
    job: &JobState,
    file: &DownloadFile,
    temp: &Path,
    counter: Arc<AtomicU64>,
) -> CoreResult<Option<chrono::DateTime<chrono::Utc>>> {
    let ctx = &job.ctx;
    let request = |range: Option<(u64, u64)>| {
        let mut req = ctx
            .clients
            .s3_transfer
            .get_object()
            .bucket(&ctx.bucket)
            .key(&file.key)
            .set_version_id(file.version_id.clone());
        // 途中でオブジェクトが置き換わらないよう、ETag で一致を確認する（バージョン指定時は不要）
        if file.version_id.is_none() && !file.etag.is_empty() {
            req = req.if_match(format!("\"{}\"", file.etag));
        }
        match range {
            Some((start, len)) => req.range(format!("bytes={start}-{}", start + len - 1)),
            None => req.checksum_mode(ChecksumMode::Enabled),
        }
    };
    let mtime_of = |metadata: Option<&std::collections::HashMap<String, String>>,
                    last_modified: Option<&aws_smithy_types::DateTime>| {
        metadata
            .and_then(|m| m.get("s3drive-mtime"))
            .and_then(|v| time::parse_rfc3339(v))
            .or_else(|| last_modified.map(time::from_aws))
    };

    if file.size < RANGED_DOWNLOAD_THRESHOLD {
        let fut = async {
            let out = request(None)
                .send()
                .await
                .map_err(|e| error_map::classify(&e, Ctx::Op("s3:GetObject")))?;
            let mtime = mtime_of(out.metadata(), out.last_modified());
            let mut out_file = tokio::fs::File::create(temp)
                .await
                .map_err(|e| CoreError::local_io(temp, &e))?;
            let mut body = out.body;
            let mut written = 0u64;
            while let Some(chunk) = body
                .try_next()
                .await
                .map_err(|e| CoreError::new(ErrorCode::Network).detail(e.to_string()))?
            {
                out_file
                    .write_all(&chunk)
                    .await
                    .map_err(|e| CoreError::local_io(temp, &e))?;
                written += chunk.len() as u64;
                counter.fetch_add(chunk.len() as u64, Ordering::Relaxed);
            }
            out_file
                .flush()
                .await
                .map_err(|e| CoreError::local_io(temp, &e))?;
            if written != file.size {
                return Err(CoreError::new(ErrorCode::Network).detail("size mismatch"));
            }
            Ok(mtime)
        };
        return tokio::select! {
            r = fut => r,
            _ = job.cancel.cancelled() => Err(CoreError::canceled()),
        };
    }

    // 64 MB 以上: 16 MiB ごとの範囲指定 GetObject を並列に行い、該当位置に書き込む
    {
        let f = tokio::fs::File::create(temp)
            .await
            .map_err(|e| CoreError::local_io(temp, &e))?;
        f.set_len(file.size)
            .await
            .map_err(|e| CoreError::local_io(temp, &e))?;
    }
    let ranges = part_ranges(file.size, DOWNLOAD_RANGE_SIZE);
    let concurrency = job.options.max_parts_per_file.max(1) as usize;
    let results: Vec<CoreResult<Option<chrono::DateTime<chrono::Utc>>>> =
        stream::iter(ranges.into_iter().enumerate())
            .map(|(i, (start, len))| {
                let counter = counter.clone();
                async move {
                    with_retry(&job.cancel, || {
                        let counter = counter.clone();
                        async move {
                            let out = request(Some((start, len)))
                                .send()
                                .await
                                .map_err(|e| error_map::classify(&e, Ctx::Op("s3:GetObject")))?;
                            let mtime = if i == 0 {
                                mtime_of(out.metadata(), out.last_modified())
                            } else {
                                None
                            };
                            let mut f = tokio::fs::OpenOptions::new()
                                .write(true)
                                .open(temp)
                                .await
                                .map_err(|e| CoreError::local_io(temp, &e))?;
                            f.seek(std::io::SeekFrom::Start(start))
                                .await
                                .map_err(|e| CoreError::local_io(temp, &e))?;
                            let mut body = out.body;
                            let mut written = 0u64;
                            let result = async {
                                while let Some(chunk) = body.try_next().await.map_err(|e| {
                                    CoreError::new(ErrorCode::Network).detail(e.to_string())
                                })? {
                                    f.write_all(&chunk)
                                        .await
                                        .map_err(|e| CoreError::local_io(temp, &e))?;
                                    written += chunk.len() as u64;
                                    counter.fetch_add(chunk.len() as u64, Ordering::Relaxed);
                                }
                                f.flush().await.map_err(|e| CoreError::local_io(temp, &e))?;
                                if written != len {
                                    return Err(CoreError::new(ErrorCode::Network)
                                        .detail("range size mismatch"));
                                }
                                Ok(())
                            }
                            .await;
                            if let Err(e) = result {
                                // 再試行で数え直すため、この範囲で数えた分を戻す
                                counter.fetch_sub(
                                    written.min(counter.load(Ordering::Relaxed)),
                                    Ordering::Relaxed,
                                );
                                return Err(e);
                            }
                            Ok(mtime)
                        }
                    })
                    .await
                }
            })
            .buffered(concurrency)
            .collect()
            .await;
    let mut mtime = None;
    for r in results {
        if let Some(m) = r? {
            mtime = Some(m);
        }
    }
    let actual = tokio::fs::metadata(temp)
        .await
        .map_err(|e| CoreError::local_io(temp, &e))?
        .len();
    if actual != file.size {
        return Err(CoreError::new(ErrorCode::Network).detail("size mismatch"));
    }
    Ok(mtime)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connections::ConnCtx;
    use crate::jobs::MemorySink;
    use crate::model::{JobStatus, TransferEvent, TransferSettings};
    use crate::store::db::tempfile_guard::TempDir;
    use crate::transfer::test_support::wait;
    use aws_sdk_s3::operation::get_object::GetObjectOutput;
    use aws_sdk_s3::operation::head_object::HeadObjectOutput;
    use aws_sdk_s3::operation::list_objects_v2::ListObjectsV2Output;
    use aws_smithy_mocks::{RuleMode, mock, mock_client};
    use aws_smithy_runtime_api::client::orchestrator::HttpResponse;
    use aws_smithy_runtime_api::http::StatusCode;
    use aws_smithy_types::body::SdkBody;
    use aws_smithy_types::byte_stream::ByteStream;

    fn object(key: &str, size: i64) -> aws_sdk_s3::types::Object {
        aws_sdk_s3::types::Object::builder()
            .key(key)
            .size(size)
            .e_tag("\"e\"")
            .last_modified(aws_smithy_types::DateTime::from_secs(1_790_000_000))
            .build()
    }

    #[tokio::test]
    async fn never_writes_outside_the_destination() {
        let list = mock!(aws_sdk_s3::Client::list_objects_v2).then_output(|| {
            ListObjectsV2Output::builder()
                .contents(object("docs/ok.txt", 2))
                // S3 のキーには .. も使える。そのまま連結すると保存先の外に書き込める（05 §3.9）
                .contents(object("docs/../../evil.txt", 4))
                .contents(object("docs/./dot.txt", 4))
                .build()
        });
        let get = mock!(aws_sdk_s3::Client::get_object)
            .match_requests(|r| r.key() == Some("docs/ok.txt"))
            .then_output(|| {
                GetObjectOutput::builder()
                    .body(ByteStream::from_static(b"ok"))
                    .build()
            });
        let s3 = mock_client!(aws_sdk_s3, RuleMode::MatchAny, [&list, &get]);
        let (core, _dir) = Core::for_tests(None).unwrap();
        core.with_test_connection(ConnCtx::for_tests("k1", "b", s3))
            .await;
        let events = Arc::new(MemorySink::<TransferEvent>::default());
        core.transfers().subscribe(events.clone());

        let root = TempDir::new().unwrap();
        let dest = root.path().join("a").join("Downloads");
        std::fs::create_dir_all(&dest).unwrap();
        let sel = core.selections().register(vec![dest.clone()]);
        let id = core
            .download_start(
                "k1",
                vec![Target::folder("docs/")],
                DownloadDestination::Selection {
                    selection_id: sel.selection_id,
                },
            )
            .await
            .unwrap();
        let job = wait(core.transfers(), &id).await;
        assert_eq!(job.status, JobStatus::Succeeded);
        assert_eq!((job.done_files, job.failed_files), (1, 2));
        assert_eq!(std::fs::read(dest.join("docs/ok.txt")).unwrap(), b"ok");
        assert!(!root.path().join("evil.txt").exists());
        assert!(!root.path().join("a/evil.txt").exists());
        assert_eq!(get.num_calls(), 1);
        let failed: Vec<_> = events
            .events()
            .into_iter()
            .filter_map(|e| match e {
                TransferEvent::FileFailed(f) => Some(f),
                _ => None,
            })
            .collect();
        assert_eq!(failed.len(), 2);
        assert!(
            failed
                .iter()
                .all(|f| f.error.code == ErrorCode::InvalidName)
        );
    }

    #[tokio::test]
    async fn restarts_once_when_the_object_is_replaced() {
        // 途中で置き換わると If-Match で 412 が返るため、HeadObject で取り直して最初からやり直す（04 §5.4）
        let stale = mock!(aws_sdk_s3::Client::get_object)
            .match_requests(|r| r.if_match() == Some("\"old\""))
            .then_http_response(|| {
                HttpResponse::new(
                    StatusCode::try_from(412).unwrap(),
                    SdkBody::from("<Error><Code>PreconditionFailed</Code></Error>"),
                )
            });
        let head = mock!(aws_sdk_s3::Client::head_object).then_output(|| {
            HeadObjectOutput::builder()
                .e_tag("\"new\"")
                .content_length(5)
                .last_modified(aws_smithy_types::DateTime::from_secs(1_790_000_000))
                .build()
        });
        let fresh = mock!(aws_sdk_s3::Client::get_object)
            .match_requests(|r| r.if_match() == Some("\"new\""))
            .then_output(|| {
                GetObjectOutput::builder()
                    .body(ByteStream::from_static(b"fresh"))
                    .build()
            });
        let s3 = mock_client!(aws_sdk_s3, RuleMode::Sequential, [&stale, &head, &fresh]);
        let ctx = Arc::new(ConnCtx::for_tests("k1", "b", s3));
        let dir = TempDir::new().unwrap();
        let (db, _db_dir) = crate::store::Db::open_temp().unwrap();
        let manager = crate::transfer::TransferManager::new(db);
        let id = manager.enqueue(
            ctx,
            TransferKind::Download,
            "「a.txt」をダウンロード中".into(),
            "~/Downloads".into(),
            TransferSettings::default(),
            vec![FileWork::Download(DownloadFile {
                key: "a.txt".into(),
                version_id: None,
                size: 3,
                etag: "old".into(),
                dir: dir.path().to_path_buf(),
                file_name: "a.txt".into(),
                is_marker: false,
                rejected: false,
            })],
        );
        let job = wait(&manager, &id).await;
        assert_eq!(job.status, JobStatus::Succeeded, "{job:?}");
        assert_eq!(std::fs::read(dir.path().join("a.txt")).unwrap(), b"fresh");
        assert_eq!(
            (stale.num_calls(), head.num_calls(), fresh.num_calls()),
            (1, 1, 1)
        );
    }

    #[test]
    fn names_version_downloads() {
        let name = version_file_name("report.pdf", "2026-09-27T05:32:00Z");
        assert!(name.starts_with("report (2026-09-27 "), "{name}");
        assert!(name.ends_with(").pdf"));
        assert!(!name.contains(':'));
    }

    #[test]
    fn shortens_home_paths() {
        if let Some(home) = std::env::var_os("HOME") {
            let p = PathBuf::from(home).join("Downloads");
            assert_eq!(display_path(&p), "~/Downloads");
        }
    }
}

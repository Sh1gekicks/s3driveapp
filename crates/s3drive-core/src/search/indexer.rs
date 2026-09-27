//! インデックスの作成（全件走査。04 §10.1）。
//!
//! ルートを区切り文字ありで列挙してから、最上位のプレフィックスごとに 4 並列で列挙する。
//! 1,000 件ずつトランザクションで書き込み、走査完了後に古い世代の行を削除する。

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use aws_sdk_s3::types::EncodingType;
use futures::{StreamExt, stream};
use rusqlite::params;
use tokio_util::sync::CancellationToken;

use super::{IndexedObject, write_object, write_prefixes};
use crate::Core;
use crate::aws::error_map::{self, Ctx};
use crate::connections::ConnCtx;
use crate::error::{CoreError, CoreResult};
use crate::jobs::{JobKind, ProgressSink};
use crate::model::{IndexEvent, IndexStatus, JobId, StorageClass};
use crate::store::Db;
use crate::util::{key, time};

const PARALLEL_PREFIXES: usize = 4;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

/// 走査中のインデックス（接続 ID → ジョブ ID と走査済み件数）。
#[derive(Default)]
pub struct IndexBuilds {
    running: std::sync::Mutex<std::collections::HashMap<String, RunningBuild>>,
}

/// 走査中のジョブ（ジョブ ID、走査済み件数、キャンセル）。
type RunningBuild = (JobId, Arc<AtomicU64>, CancellationToken);

/// インデックスの更新完了を通知する先（`index://updated` イベント）。
pub type IndexListener = dyn Fn(&str, &IndexStatus) + Send + Sync;

pub(crate) struct IndexRunner<'a> {
    core: &'a Core,
}

impl<'a> IndexRunner<'a> {
    pub fn new(core: &'a Core) -> Self {
        Self { core }
    }

    pub fn progress(&self, connection_id: &str) -> Option<u64> {
        self.core
            .0
            .index_builds
            .running
            .lock()
            .unwrap()
            .get(connection_id)
            .map(|(_, n, _)| n.load(Ordering::Relaxed))
    }

    pub fn cancel(&self, connection_id: &str) {
        if let Some((_, _, token)) = self
            .core
            .0
            .index_builds
            .running
            .lock()
            .unwrap()
            .get(connection_id)
        {
            token.cancel();
        }
    }

    /// 走査を始める。すでに走査中ならそのジョブ ID を返す。
    pub fn start(&self, ctx: Arc<ConnCtx>, sink: Arc<dyn ProgressSink<IndexEvent>>) -> JobId {
        let connection_id = ctx.id.clone();
        let mut running = self.core.0.index_builds.running.lock().unwrap();
        if let Some((job_id, _, _)) = running.get(&connection_id) {
            return job_id.clone();
        }
        let (job_id, cancel) = self
            .core
            .jobs()
            .register(JobKind::IndexBuild, Some(&connection_id));
        let scanned = Arc::new(AtomicU64::new(0));
        running.insert(
            connection_id.clone(),
            (job_id.clone(), scanned.clone(), cancel.clone()),
        );
        drop(running);

        let core = self.core.clone();
        let id = job_id.clone();
        tokio::spawn(async move {
            let result = build(&core.0.db, &ctx, &id, scanned, &cancel, sink.as_ref()).await;
            core.0.index_builds.running.lock().unwrap().remove(&ctx.id);
            core.jobs().finish(&id);
            let auto = core
                .0
                .settings
                .read(|f| f.settings.search.auto_refresh_minutes);
            match result {
                Ok(()) => {
                    let status = super::status(&core.0.db, &ctx.id, auto, None).await;
                    if let Ok(status) = status {
                        sink.send(IndexEvent::Finished {
                            job_id: id.clone(),
                            status: status.clone(),
                        });
                        if let Some(listener) = core.0.index_listener.lock().unwrap().as_ref() {
                            listener(&ctx.id, &status);
                        }
                    }
                }
                Err(e) => {
                    if !e.is_canceled() {
                        log::warn!("インデックスを作成できませんでした: {e}");
                    }
                    sink.send(IndexEvent::Failed {
                        job_id: id,
                        error: e.into(),
                    });
                }
            }
        });
        job_id
    }
}

impl Core {
    /// 背景でのインデックス更新の完了を受け取る関数を設定する。
    pub fn set_index_listener(&self, listener: Box<IndexListener>) {
        *self.0.index_listener.lock().unwrap() = Some(listener);
    }
}

struct Progress<'a> {
    job_id: &'a str,
    scanned: Arc<AtomicU64>,
    sink: &'a dyn ProgressSink<IndexEvent>,
    last: std::sync::Mutex<Instant>,
}

impl Progress<'_> {
    fn add(&self, n: u64) {
        let total = self.scanned.fetch_add(n, Ordering::Relaxed) + n;
        let mut last = self.last.lock().unwrap();
        if last.elapsed() >= PROGRESS_INTERVAL {
            *last = Instant::now();
            self.sink.send(IndexEvent::Progress {
                job_id: self.job_id.to_string(),
                scanned: total,
            });
        }
    }
}

async fn build(
    db: &Db,
    ctx: &ConnCtx,
    job_id: &str,
    scanned: Arc<AtomicU64>,
    cancel: &CancellationToken,
    sink: &dyn ProgressSink<IndexEvent>,
) -> CoreResult<()> {
    let cid = ctx.id.clone();
    let generation: i64 = db
        .run(move |c| {
            c.execute(
                "INSERT INTO index_state (connection_id, status, generation) VALUES (?1, 'building', 1)
                 ON CONFLICT (connection_id) DO UPDATE SET status = 'building', generation = generation + 1",
                [&cid],
            )?;
            Ok(c.query_row("SELECT generation FROM index_state WHERE connection_id = ?1", [&cid], |r| r.get(0))?)
        })
        .await?;
    let progress = Progress {
        job_id,
        scanned,
        sink,
        last: std::sync::Mutex::new(Instant::now() - PROGRESS_INTERVAL),
    };

    let scan = async {
        // ルートを区切り文字ありで列挙する
        let mut top_prefixes = Vec::new();
        let mut token = None;
        loop {
            if cancel.is_cancelled() {
                return Err(CoreError::canceled());
            }
            let out = ctx
                .clients
                .s3_bulk
                .list_objects_v2()
                .bucket(&ctx.bucket)
                .delimiter("/")
                .max_keys(1000)
                .encoding_type(EncodingType::Url)
                .set_continuation_token(token)
                .send()
                .await
                .map_err(|e| error_map::classify(&e, Ctx::Op("s3:ListBucket")))?;
            let objects: Vec<IndexedObject> =
                out.contents().iter().filter_map(indexed_from).collect();
            progress.add(objects.len() as u64);
            write_batch(db, &ctx.id, objects, generation).await?;
            top_prefixes.extend(
                out.common_prefixes()
                    .iter()
                    .filter_map(|p| p.prefix())
                    .map(key::url_decode_key),
            );
            token = out.next_continuation_token().map(str::to_string);
            if token.is_none() {
                break;
            }
        }
        // 最上位のプレフィックスごとに並列で列挙する
        let results: Vec<CoreResult<()>> = stream::iter(top_prefixes)
            .map(|prefix| scan_prefix(db, ctx, prefix, generation, &progress, cancel))
            .buffer_unordered(PARALLEL_PREFIXES)
            .collect()
            .await;
        results.into_iter().collect::<CoreResult<()>>()
    };

    let outcome = scan.await;
    let cid = ctx.id.clone();
    let ok = outcome.is_ok();
    db.run(move |c| {
        let tx = c.transaction()?;
        if ok {
            // 走査で見つからなかった（削除された）キーを消す
            tx.execute("DELETE FROM objects WHERE connection_id = ?1 AND generation < ?2", params![cid, generation])?;
            tx.execute("DELETE FROM prefixes WHERE connection_id = ?1 AND generation < ?2", params![cid, generation])?;
            tx.execute(
                "UPDATE index_state SET status = 'ready', last_full_scan_at = ?2 WHERE connection_id = ?1",
                params![cid, time::now_rfc3339()],
            )?;
        } else {
            // 途中までの結果は残し、次の検索で走査し直す
            tx.execute("UPDATE index_state SET status = 'ready' WHERE connection_id = ?1", params![cid])?;
        }
        super::refresh_counts(&tx, &cid)?;
        tx.commit()?;
        Ok(())
    })
    .await?;
    outcome
}

async fn scan_prefix(
    db: &Db,
    ctx: &ConnCtx,
    prefix: String,
    generation: i64,
    progress: &Progress<'_>,
    cancel: &CancellationToken,
) -> CoreResult<()> {
    let mut token = None;
    loop {
        if cancel.is_cancelled() {
            return Err(CoreError::canceled());
        }
        let out = ctx
            .clients
            .s3_bulk
            .list_objects_v2()
            .bucket(&ctx.bucket)
            .prefix(&prefix)
            .max_keys(1000)
            .encoding_type(EncodingType::Url)
            .set_continuation_token(token)
            .send()
            .await
            .map_err(|e| error_map::classify(&e, Ctx::Op("s3:ListBucket")))?;
        let objects: Vec<IndexedObject> = out.contents().iter().filter_map(indexed_from).collect();
        if objects.is_empty() && out.next_continuation_token().is_none() {
            // オブジェクトのないプレフィックス（通常は起きない）もフォルダとして残す
            let (cid, p) = (ctx.id.clone(), prefix.clone());
            db.run(move |c| {
                let tx = c.transaction()?;
                write_prefixes(&tx, &cid, &p, generation, &mut BTreeSet::new())?;
                tx.commit()?;
                Ok(())
            })
            .await?;
        }
        progress.add(objects.len() as u64);
        write_batch(db, &ctx.id, objects, generation).await?;
        token = out.next_continuation_token().map(str::to_string);
        if token.is_none() {
            return Ok(());
        }
    }
}

fn indexed_from(o: &aws_sdk_s3::types::Object) -> Option<IndexedObject> {
    Some(IndexedObject {
        key: key::url_decode_key(o.key()?),
        size: o.size().unwrap_or(0).max(0) as u64,
        last_modified: o
            .last_modified()
            .map(time::aws_to_rfc3339)
            .unwrap_or_default(),
        etag: Some(crate::objects::trim_etag(o.e_tag())),
        storage_class: StorageClass::from_s3(o.storage_class().map(|c| c.as_str())),
    })
}

async fn write_batch(
    db: &Db,
    connection_id: &str,
    objects: Vec<IndexedObject>,
    generation: i64,
) -> CoreResult<()> {
    if objects.is_empty() {
        return Ok(());
    }
    let cid = connection_id.to_string();
    db.run(move |c| {
        let tx = c.transaction()?;
        let mut seen = BTreeSet::new();
        for o in &objects {
            write_object(&tx, &cid, o, generation)?;
            write_prefixes(&tx, &cid, &o.key, generation, &mut seen)?;
        }
        tx.commit()?;
        Ok(())
    })
    .await
}

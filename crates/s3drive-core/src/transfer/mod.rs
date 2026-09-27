//! 転送管理（04 §14）。
//!
//! アップロードとダウンロードは同じ FIFO のキューで扱い、ファイル単位の並列数（設定。既定 3）を守る。
//! 進捗は 100 ms ごとに間引いて `TransferEvent` で送る。

mod body;
mod download;
pub mod multipart;
mod upload;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::time::{Duration, Instant};

use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::connections::ConnCtx;
use crate::error::{CoreError, CoreResult};
use crate::jobs::ProgressSink;
use crate::model::{
    FileFailed, JobId, JobStatus, TransferEvent, TransferJob, TransferKind, TransferSettings,
};
use crate::store::Db;

pub use download::DownloadFile;
pub use upload::{UploadFile, UploadPlanData};

const TICK: Duration = Duration::from_millis(100);
const SPEED_WINDOW: Duration = Duration::from_secs(5);
const PLAN_TTL: Duration = Duration::from_secs(10 * 60);

/// 1 ファイルの転送。
#[derive(Debug, Clone)]
pub(crate) enum FileWork {
    Upload(UploadFile),
    Download(DownloadFile),
}

impl FileWork {
    fn name(&self) -> String {
        match self {
            Self::Upload(f) => f.display_name(),
            Self::Download(f) => f.display_name(),
        }
    }

    fn size(&self) -> u64 {
        match self {
            Self::Upload(f) => f.size,
            Self::Download(f) => f.size,
        }
    }
}

/// 実行中のジョブの状態。
pub(crate) struct JobState {
    pub id: JobId,
    pub ctx: Arc<ConnCtx>,
    pub cancel: CancellationToken,
    pub options: TransferSettings,
    snapshot: Mutex<TransferJob>,
    finished_bytes: AtomicU64,
    active: Mutex<Vec<Arc<AtomicU64>>>,
    pending: AtomicUsize,
    samples: Mutex<VecDeque<(Instant, u64)>>,
    failed: Mutex<Vec<FileWork>>,
    pub(crate) saved: Mutex<Vec<PathBuf>>,
    pub(crate) reserved_paths: Mutex<HashSet<PathBuf>>,
    last_emitted: AtomicU64,
}

impl JobState {
    fn done_bytes(&self) -> u64 {
        let active: u64 = self
            .active
            .lock()
            .unwrap()
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .sum();
        self.finished_bytes.load(Ordering::Relaxed) + active
    }

    /// 送信・受信済みのバイト数、速度（直近 5 秒の移動平均）、残り時間を反映した状態。
    fn snapshot(&self) -> TransferJob {
        let mut job = self.snapshot.lock().unwrap().clone();
        let done = self.done_bytes().min(job.total_bytes);
        job.done_bytes = done;
        let now = Instant::now();
        let mut samples = self.samples.lock().unwrap();
        samples.push_back((now, done));
        while samples
            .front()
            .is_some_and(|(t, _)| now.duration_since(*t) > SPEED_WINDOW)
        {
            samples.pop_front();
        }
        if let (Some((t0, b0)), Some((t1, b1))) = (samples.front(), samples.back()) {
            let secs = t1.duration_since(*t0).as_secs_f64();
            job.bytes_per_sec = if secs > 0.5 {
                (b1.saturating_sub(*b0)) as f64 / secs
            } else {
                0.0
            };
        }
        job.eta_sec = (job.status == JobStatus::Running && job.bytes_per_sec > 1.0)
            .then(|| ((job.total_bytes - done) as f64 / job.bytes_per_sec).ceil() as u64);
        job
    }

    fn update(&self, f: impl FnOnce(&mut TransferJob)) {
        f(&mut self.snapshot.lock().unwrap());
    }

    fn status(&self) -> JobStatus {
        self.snapshot.lock().unwrap().status
    }
}

#[derive(Default)]
struct Shared {
    jobs: Mutex<Vec<Arc<JobState>>>,
    queue: Mutex<VecDeque<(Arc<JobState>, FileWork)>>,
    running: AtomicUsize,
    max_files: AtomicUsize,
    notify: Notify,
    sink: Mutex<Option<Arc<dyn ProgressSink<TransferEvent>>>>,
    plans: Mutex<HashMap<String, (Instant, UploadPlanData)>>,
}

pub struct TransferManager {
    db: Db,
    shared: Arc<Shared>,
    started: Once,
}

impl TransferManager {
    pub fn new(db: Db) -> Self {
        let shared = Shared {
            max_files: AtomicUsize::new(3),
            ..Default::default()
        };
        Self {
            db,
            shared: Arc::new(shared),
            started: Once::new(),
        }
    }

    /// 以後の更新の送り先を設定し、現在のジョブ一覧を返す（`transfer_subscribe`）。
    pub fn subscribe(&self, sink: Arc<dyn ProgressSink<TransferEvent>>) -> Vec<TransferJob> {
        *self.shared.sink.lock().unwrap() = Some(sink);
        self.jobs()
    }

    pub fn jobs(&self) -> Vec<TransferJob> {
        self.shared
            .jobs
            .lock()
            .unwrap()
            .iter()
            .map(|j| j.snapshot())
            .collect()
    }

    /// 設定「同時に転送するファイル数」を反映する。
    pub fn set_max_files(&self, n: u32) {
        self.shared
            .max_files
            .store(n.clamp(1, 8) as usize, Ordering::Relaxed);
        self.shared.notify.notify_one();
    }

    /// 転送中のジョブ数と残りのバイト数（メニューバー常駐の状態表示・終了時の確認に使う）。
    pub fn summary(&self) -> (usize, u64) {
        let jobs = self.shared.jobs.lock().unwrap();
        let active: Vec<_> = jobs.iter().filter(|j| !j.status().is_finished()).collect();
        let remaining = active
            .iter()
            .map(|j| {
                let s = j.snapshot.lock().unwrap().total_bytes;
                s.saturating_sub(j.done_bytes())
            })
            .sum();
        (active.len(), remaining)
    }

    /// 完了済みのジョブを一覧から消す。
    pub fn clear_finished(&self) {
        self.shared
            .jobs
            .lock()
            .unwrap()
            .retain(|j| !j.status().is_finished());
    }

    pub(crate) fn job(&self, job_id: &str) -> Option<Arc<JobState>> {
        self.shared
            .jobs
            .lock()
            .unwrap()
            .iter()
            .find(|j| j.id == job_id)
            .cloned()
    }

    /// 保存したファイル（「Finder に表示」に使う）。
    pub fn saved_paths(&self, job_id: &str) -> Vec<PathBuf> {
        self.job(job_id)
            .map(|j| j.saved.lock().unwrap().clone())
            .unwrap_or_default()
    }

    /// 転送をキャンセルする。転送のジョブでなければ偽。
    pub fn cancel(&self, job_id: &str) -> bool {
        match self.job(job_id) {
            Some(job) => {
                job.cancel.cancel();
                true
            }
            None => false,
        }
    }

    /// 接続の削除時に、その接続の転送をキャンセルする。
    pub fn cancel_connection(&self, connection_id: &str) {
        for job in self.shared.jobs.lock().unwrap().iter() {
            if job.ctx.id == connection_id {
                job.cancel.cancel();
            }
        }
    }

    /// サインアウト時に全転送をキャンセルし、一覧を空にする。
    pub fn cancel_all(&self) {
        let mut jobs = self.shared.jobs.lock().unwrap();
        for job in jobs.iter() {
            job.cancel.cancel();
        }
        jobs.retain(|j| !j.status().is_finished());
    }

    /// すべての転送を中止し、中止処理を最大 `timeout` 待つ（04 §14.4）。
    pub async fn shutdown(&self, timeout: Duration) {
        for job in self.shared.jobs.lock().unwrap().iter() {
            job.cancel.cancel();
        }
        let deadline = Instant::now() + timeout;
        while self.shared.running.load(Ordering::Relaxed) > 0 && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    pub(crate) fn store_plan(&self, plan_id: String, plan: UploadPlanData) {
        let mut plans = self.shared.plans.lock().unwrap();
        plans.retain(|_, (t, _)| t.elapsed() < PLAN_TTL);
        plans.insert(plan_id, (Instant::now(), plan));
    }

    pub(crate) fn take_plan(&self, plan_id: &str) -> CoreResult<UploadPlanData> {
        self.shared
            .plans
            .lock()
            .unwrap()
            .remove(plan_id)
            .filter(|(t, _)| t.elapsed() < PLAN_TTL)
            .map(|(_, p)| p)
            .ok_or_else(|| {
                CoreError::with_message(
                    crate::ErrorCode::NotFound,
                    "アップロードの準備が無効になりました。もう一度お試しください",
                )
            })
    }

    fn emit(&self, event: TransferEvent) {
        emit(&self.shared, event);
    }

    /// ジョブを登録してキューに積む。
    pub(crate) fn enqueue(
        &self,
        ctx: Arc<ConnCtx>,
        kind: TransferKind,
        title: String,
        destination: String,
        options: TransferSettings,
        files: Vec<FileWork>,
    ) -> JobId {
        let id = uuid::Uuid::new_v4().to_string();
        let cancel = CancellationToken::new();
        let total_bytes = files.iter().map(FileWork::size).sum();
        let job = Arc::new(JobState {
            id: id.clone(),
            snapshot: Mutex::new(TransferJob {
                job_id: id.clone(),
                kind,
                connection_id: ctx.id.clone(),
                title,
                status: if files.is_empty() {
                    JobStatus::Succeeded
                } else {
                    JobStatus::Queued
                },
                total_files: files.len() as u64,
                done_files: 0,
                failed_files: 0,
                total_bytes,
                done_bytes: 0,
                current_name: None,
                bytes_per_sec: 0.0,
                eta_sec: None,
                destination,
            }),
            ctx,
            cancel,
            options,
            finished_bytes: AtomicU64::new(0),
            active: Mutex::new(Vec::new()),
            pending: AtomicUsize::new(files.len()),
            samples: Mutex::new(VecDeque::new()),
            failed: Mutex::new(Vec::new()),
            saved: Mutex::new(Vec::new()),
            reserved_paths: Mutex::new(HashSet::new()),
            last_emitted: AtomicU64::new(u64::MAX),
        });
        self.shared.jobs.lock().unwrap().push(job.clone());
        self.emit(TransferEvent::JobUpdated(job.snapshot()));
        if files.is_empty() {
            return id;
        }
        {
            let mut queue = self.shared.queue.lock().unwrap();
            for f in files {
                queue.push_back((job.clone(), f));
            }
        }
        self.start_workers();
        self.shared.notify.notify_one();
        id
    }

    /// 失敗したファイルだけを再実行する新しいジョブを作る。
    pub(crate) fn retry_files(&self, job_id: &str) -> CoreResult<(Arc<JobState>, Vec<FileWork>)> {
        let job = self.job(job_id).ok_or_else(|| {
            CoreError::with_message(crate::ErrorCode::NotFound, "転送が見つかりません")
        })?;
        let files = std::mem::take(&mut *job.failed.lock().unwrap());
        Ok((job, files))
    }

    fn start_workers(&self) {
        self.started.call_once(|| {
            tokio::spawn(dispatcher(self.shared.clone(), self.db.clone()));
            tokio::spawn(ticker(self.shared.clone()));
        });
    }

    pub(crate) fn job_snapshot(&self, job_id: &str) -> Option<TransferJob> {
        self.job(job_id).map(|j| j.snapshot())
    }
}

fn emit(shared: &Shared, event: TransferEvent) {
    if let Some(sink) = shared.sink.lock().unwrap().as_ref() {
        sink.send(event);
    }
}

/// キューからファイルを取り出して実行する。
async fn dispatcher(shared: Arc<Shared>, db: Db) {
    loop {
        loop {
            if shared.running.load(Ordering::Relaxed) >= shared.max_files.load(Ordering::Relaxed) {
                break;
            }
            let Some((job, work)) = shared.queue.lock().unwrap().pop_front() else {
                break;
            };
            if job.cancel.is_cancelled() {
                file_finished(&shared, &job, &work, None, Err(CoreError::canceled()));
                continue;
            }
            shared.running.fetch_add(1, Ordering::Relaxed);
            job.update(|j| {
                j.status = JobStatus::Running;
                j.current_name = Some(work.name());
            });
            emit(&shared, TransferEvent::JobUpdated(job.snapshot()));
            let (shared, db) = (shared.clone(), db.clone());
            tokio::spawn(async move {
                let counter = Arc::new(AtomicU64::new(0));
                job.active.lock().unwrap().push(counter.clone());
                let result = match &work {
                    FileWork::Upload(f) => upload::run(&db, &job, f, counter.clone()).await,
                    FileWork::Download(f) => download::run(&job, f, counter.clone()).await,
                };
                file_finished(&shared, &job, &work, Some(&counter), result);
                shared.running.fetch_sub(1, Ordering::Relaxed);
                shared.notify.notify_one();
            });
        }
        shared.notify.notified().await;
    }
}

fn file_finished(
    shared: &Shared,
    job: &Arc<JobState>,
    work: &FileWork,
    counter: Option<&Arc<AtomicU64>>,
    result: CoreResult<()>,
) {
    if let Some(c) = counter {
        job.active.lock().unwrap().retain(|a| !Arc::ptr_eq(a, c));
    }
    match &result {
        Ok(()) => {
            job.finished_bytes.fetch_add(work.size(), Ordering::Relaxed);
            job.update(|j| j.done_files += 1);
        }
        Err(e) if e.is_canceled() => {}
        Err(e) => {
            job.update(|j| j.failed_files += 1);
            job.failed.lock().unwrap().push(work.clone());
            emit(
                shared,
                TransferEvent::FileFailed(FileFailed {
                    job_id: job.id.clone(),
                    name: work.name(),
                    error: e.clone().into(),
                }),
            );
            log::warn!("転送に失敗しました: {e}");
        }
    }
    if job.pending.fetch_sub(1, Ordering::Relaxed) == 1 {
        job.update(|j| {
            j.current_name = None;
            j.status = if job.cancel.is_cancelled() {
                JobStatus::Canceled
            } else if j.failed_files > 0 && j.failed_files == j.total_files {
                JobStatus::Failed
            } else {
                // 一部成功は succeeded に失敗件数を付ける（01 §6.2）
                JobStatus::Succeeded
            };
        });
    }
    emit(shared, TransferEvent::JobUpdated(job.snapshot()));
}

/// 実行中のジョブの進捗を 100 ms ごとに送る（変化があった場合のみ）。
async fn ticker(shared: Arc<Shared>) {
    let mut interval = tokio::time::interval(TICK);
    loop {
        interval.tick().await;
        let jobs: Vec<Arc<JobState>> = shared
            .jobs
            .lock()
            .unwrap()
            .iter()
            .filter(|j| j.status() == JobStatus::Running)
            .cloned()
            .collect();
        for job in jobs {
            let done = job.done_bytes();
            if job.last_emitted.swap(done, Ordering::Relaxed) != done {
                emit(&shared, TransferEvent::JobUpdated(job.snapshot()));
            }
        }
    }
}

/// パートごとの再試行（最大 5 回、指数バックオフ。01 §7.2）。キャンセルされたら即座に止める。
pub(crate) async fn with_retry<T, F, Fut>(cancel: &CancellationToken, mut f: F) -> CoreResult<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = CoreResult<T>>,
{
    const ATTEMPTS: u32 = 5;
    let mut attempt = 0;
    loop {
        let result = tokio::select! {
            r = f() => r,
            _ = cancel.cancelled() => return Err(CoreError::canceled()),
        };
        match result {
            Ok(v) => return Ok(v),
            Err(e) if e.code.retryable() && attempt + 1 < ATTEMPTS => {
                let delay = crate::util::backoff_delay(attempt);
                attempt += 1;
                log::debug!("再試行します（{attempt} 回目、{delay:?} 後）: {e}");
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    _ = cancel.cancelled() => return Err(CoreError::canceled()),
                }
            }
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCode;

    #[tokio::test(start_paused = true)]
    async fn retries_retryable_errors_up_to_five_times() {
        let cancel = CancellationToken::new();
        let mut calls = 0;
        let r: CoreResult<()> = with_retry(&cancel, || {
            calls += 1;
            async { Err(CoreError::new(ErrorCode::Network)) }
        })
        .await;
        assert_eq!(r.unwrap_err().code, ErrorCode::Network);
        assert_eq!(calls, 5);
    }

    #[tokio::test(start_paused = true)]
    async fn does_not_retry_permanent_errors() {
        let cancel = CancellationToken::new();
        let mut calls = 0;
        let r: CoreResult<()> = with_retry(&cancel, || {
            calls += 1;
            async { Err(CoreError::new(ErrorCode::AccessDenied)) }
        })
        .await;
        assert!(r.is_err());
        assert_eq!(calls, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn succeeds_after_transient_failures() {
        let cancel = CancellationToken::new();
        let mut calls = 0;
        let r = with_retry(&cancel, || {
            calls += 1;
            let n = calls;
            async move {
                if n < 3 {
                    Err(CoreError::new(ErrorCode::SlowDown))
                } else {
                    Ok(n)
                }
            }
        })
        .await;
        assert_eq!(r.unwrap(), 3);
    }
}

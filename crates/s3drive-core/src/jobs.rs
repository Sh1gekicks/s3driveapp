//! ジョブの登録とキャンセル（01 §6.2、05 §7）。

use std::collections::HashMap;
use std::sync::Mutex;

use tokio_util::sync::CancellationToken;

use crate::model::{ConnectionId, JobId};

/// ジョブの進捗を通知する先。`src-tauri` は `tauri::ipc::Channel` で実装する（05 §1.1）。
pub trait ProgressSink<E>: Send + Sync + 'static {
    fn send(&self, event: E);
}

/// 何もしない通知先。
pub struct NullSink;

impl<E> ProgressSink<E> for NullSink {
    fn send(&self, _event: E) {}
}

/// テスト用。受け取ったイベントを保持する。
pub struct MemorySink<E>(pub Mutex<Vec<E>>);

impl<E> Default for MemorySink<E> {
    fn default() -> Self {
        Self(Mutex::new(Vec::new()))
    }
}

impl<E: Send + 'static> ProgressSink<E> for MemorySink<E> {
    fn send(&self, event: E) {
        self.0.lock().unwrap().push(event);
    }
}

impl<E: Clone> MemorySink<E> {
    pub fn events(&self) -> Vec<E> {
        self.0.lock().unwrap().clone()
    }
}

/// ジョブの種類（01 §6.2）。取り出しの要求とメトリクス・コストの取得は、コマンドの戻り値で結果を返すためジョブにしない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    Upload,
    Download,
    Delete,
    Move,
    StorageClass,
    IndexBuild,
}

struct JobEntry {
    token: CancellationToken,
    connection_id: Option<ConnectionId>,
    kind: JobKind,
}

#[derive(Default)]
pub struct JobRegistry {
    jobs: Mutex<HashMap<JobId, JobEntry>>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &self,
        kind: JobKind,
        connection_id: Option<&str>,
    ) -> (JobId, CancellationToken) {
        let id = uuid::Uuid::new_v4().to_string();
        let token = CancellationToken::new();
        self.jobs.lock().unwrap().insert(
            id.clone(),
            JobEntry {
                token: token.clone(),
                connection_id: connection_id.map(str::to_string),
                kind,
            },
        );
        (id, token)
    }

    /// ジョブの終了時に呼ぶ。
    pub fn finish(&self, id: &str) {
        self.jobs.lock().unwrap().remove(id);
    }

    /// キャンセルする。登録されていないジョブなら偽。
    pub fn cancel(&self, id: &str) -> bool {
        match self.jobs.lock().unwrap().get(id) {
            Some(job) => {
                job.token.cancel();
                true
            }
            None => false,
        }
    }

    /// サインアウト時に全ジョブをキャンセルする。
    pub fn cancel_all(&self) {
        for job in self.jobs.lock().unwrap().values() {
            job.token.cancel();
        }
    }

    /// 接続の削除時にその接続のジョブをキャンセルする。
    pub fn cancel_connection(&self, connection_id: &str) {
        for job in self.jobs.lock().unwrap().values() {
            if job.connection_id.as_deref() == Some(connection_id) {
                job.token.cancel();
            }
        }
    }

    pub fn count(&self, kinds: &[JobKind]) -> usize {
        self.jobs
            .lock()
            .unwrap()
            .values()
            .filter(|j| kinds.contains(&j.kind))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancels_jobs_individually_and_by_connection() {
        let jobs = JobRegistry::new();
        let (a, ta) = jobs.register(JobKind::Upload, Some("c1"));
        let (_b, tb) = jobs.register(JobKind::Delete, Some("c2"));
        let (_c, tc) = jobs.register(JobKind::Download, Some("c1"));
        assert!(jobs.cancel(&a));
        assert!(ta.is_cancelled() && !tb.is_cancelled());
        jobs.cancel_connection("c1");
        assert!(tc.is_cancelled() && !tb.is_cancelled());
        assert_eq!(jobs.count(&[JobKind::Upload, JobKind::Download]), 2);
        jobs.finish(&a);
        assert!(!jobs.cancel(&a));
        jobs.cancel_all();
        assert!(tb.is_cancelled());
    }
}

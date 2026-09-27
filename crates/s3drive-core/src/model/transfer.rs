use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::common::{ConnectionId, JobId};
use super::object::RemoteConflict;
use crate::error::AppError;

/// ファイル選択・ドロップで得たローカルの項目（05 §3.9）。パスはフロントエンドに渡さない。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SelectionItem {
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Selection {
    pub selection_id: String,
    pub items: Vec<SelectionItem>,
}

/// アップロードから除外した理由（04 §4.4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExcludeReason {
    Symlink,
    Ignored,
    KeyTooLong,
    InvalidChar,
    Unreadable,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExcludedItem {
    pub name: String,
    pub reason: ExcludeReason,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UploadConflict {
    pub key: String,
    pub local_size: u64,
    pub remote_size: u64,
    pub remote_modified: String,
}

impl UploadConflict {
    pub fn new(remote: RemoteConflict, local_size: u64) -> Self {
        Self {
            key: remote.key,
            local_size,
            remote_size: remote.remote_size,
            remote_modified: remote.remote_modified,
        }
    }
}

/// アップロードの計画（04 §4.1）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UploadPlan {
    pub plan_id: String,
    pub file_count: u64,
    pub total_bytes: u64,
    pub conflicts: Vec<UploadConflict>,
    pub excluded: Vec<ExcludedItem>,
    /// バケットのバージョニングが有効か（DLG-08 の説明文）。
    pub versioning_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TransferKind {
    Upload,
    Download,
}

/// ジョブの状態（01 §6.2、04 §14.1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Canceled,
}

impl JobStatus {
    pub fn is_finished(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Canceled)
    }
}

/// 転送ジョブ（ユーザーの 1 回の操作）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TransferJob {
    pub job_id: JobId,
    pub kind: TransferKind,
    pub connection_id: ConnectionId,
    pub title: String,
    pub status: JobStatus,
    pub total_files: u64,
    pub done_files: u64,
    pub failed_files: u64,
    pub total_bytes: u64,
    pub done_bytes: u64,
    pub current_name: Option<String>,
    pub bytes_per_sec: f64,
    pub eta_sec: Option<u64>,
    /// 転送先の表示（「{バケット}/{プレフィックス}」または保存先フォルダ名）。
    pub destination: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileFailed {
    pub job_id: JobId,
    pub name: String,
    pub error: AppError,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
#[ts(export)]
pub enum TransferEvent {
    JobUpdated(TransferJob),
    FileFailed(FileFailed),
}

/// ダウンロードの保存先（既定のダウンロード先、または選択したフォルダ）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum DownloadDestination {
    Keyword(DestinationKeyword),
    #[serde(rename_all = "camelCase")]
    Selection {
        selection_id: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DestinationKeyword {
    Default,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_destination_accepts_keyword_and_selection() {
        let d: DownloadDestination = serde_json::from_str(r#""default""#).unwrap();
        assert!(matches!(
            d,
            DownloadDestination::Keyword(DestinationKeyword::Default)
        ));
        let d: DownloadDestination = serde_json::from_str(r#"{"selectionId":"s1"}"#).unwrap();
        assert!(
            matches!(d, DownloadDestination::Selection { selection_id } if selection_id == "s1")
        );
    }
}

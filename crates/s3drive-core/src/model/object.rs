use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::common::{JobId, StorageClass, Timestamp};
use crate::error::AppError;

/// アーカイブの取り出し状態（04 §8.4）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum RestoreState {
    NotArchived,
    /// 取り出しが必要。
    Archived,
    InProgress,
    Restored {
        expiry: Timestamp,
    },
}

impl RestoreState {
    /// ダウンロード・コピーができない状態（取り出していないアーカイブ）。
    pub fn needs_restore(&self) -> bool {
        matches!(self, Self::Archived | Self::InProgress)
    }
}

/// 一覧の項目（05 §2）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum Entry {
    #[serde(rename_all = "camelCase")]
    Folder {
        key: String,
        name: String,
        last_modified: Option<Timestamp>,
        deleted: bool,
    },
    #[serde(rename_all = "camelCase")]
    File {
        key: String,
        name: String,
        size: u64,
        last_modified: Timestamp,
        etag: String,
        storage_class: StorageClass,
        restore: RestoreState,
        deleted: bool,
    },
}

impl Entry {
    pub fn key(&self) -> &str {
        match self {
            Self::Folder { key, .. } | Self::File { key, .. } => key,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Folder { name, .. } | Self::File { name, .. } => name,
        }
    }

    pub fn is_folder(&self) -> bool {
        matches!(self, Self::Folder { .. })
    }
}

/// 一覧の 1 ページ。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListPage {
    pub entries: Vec<Entry>,
    pub next_token: Option<String>,
}

/// 一覧の取得条件。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListOptions {
    pub show_hidden: bool,
    pub include_deleted: bool,
}

/// 作成日の決め方（04 §9.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CreatedSource {
    OldestVersion,
    Metadata,
    LastModified,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CreatedInfo {
    pub at: Timestamp,
    pub source: CreatedSource,
}

/// オブジェクトのメタデータ（HeadObject）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ObjectDetail {
    pub key: String,
    pub version_id: Option<String>,
    pub size: u64,
    pub content_type: String,
    pub last_modified: Timestamp,
    pub created: CreatedInfo,
    pub etag: String,
    pub storage_class: StorageClass,
    pub encryption: String,
    pub kms_key_id: Option<String>,
    pub checksums: BTreeMap<String, String>,
    pub user_metadata: BTreeMap<String, String>,
    pub restore: RestoreState,
}

/// オブジェクトのバージョン。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ObjectVersion {
    pub version_id: String,
    pub is_latest: bool,
    pub is_delete_marker: bool,
    pub last_modified: Timestamp,
    pub size: Option<u64>,
    pub storage_class: Option<StorageClass>,
    pub etag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VersionPage {
    pub versions: Vec<ObjectVersion>,
    pub next_cursor: Option<String>,
}

/// 操作の対象。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Target {
    pub key: String,
    pub is_folder: bool,
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
}

impl Target {
    pub fn file(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            is_folder: false,
            version_id: None,
        }
    }

    pub fn folder(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            is_folder: true,
            version_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SkippedItem {
    pub key: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FailedItem {
    pub key: String,
    pub error: AppError,
}

/// 一括操作の結果。
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BatchResult {
    pub succeeded: u64,
    pub skipped: Vec<SkippedItem>,
    pub failed: Vec<FailedItem>,
}

impl BatchResult {
    pub fn skip(&mut self, key: impl Into<String>, reason: impl Into<String>) {
        self.skipped.push(SkippedItem {
            key: key.into(),
            reason: reason.into(),
        });
    }

    pub fn fail(&mut self, key: impl Into<String>, error: crate::error::CoreError) {
        self.failed.push(FailedItem {
            key: key.into(),
            error: error.into(),
        });
    }

    pub fn merge(&mut self, other: BatchResult) {
        self.succeeded += other.succeeded;
        self.skipped.extend(other.skipped);
        self.failed.extend(other.failed);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BatchProgress {
    pub job_id: JobId,
    pub done: u64,
    pub total: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BatchFinished {
    pub job_id: JobId,
    pub result: BatchResult,
}

/// 一括操作（削除・移動・クラス変更）の進捗と結果。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
#[ts(export)]
pub enum BatchEvent {
    Progress(BatchProgress),
    Finished(BatchFinished),
}

/// フォルダの項目数と合計サイズ。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderSummary {
    pub item_count: u64,
    pub total_bytes: u64,
    /// 列挙の上限（10 万件）に達した。
    pub truncated: bool,
}

/// 移動先・アップロード先の同名の項目。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RemoteConflict {
    pub key: String,
    pub remote_size: u64,
    pub remote_modified: Timestamp,
}

/// 同名の項目の扱い（DLG-08）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ConflictDecision {
    Replace,
    Skip,
    KeepBoth,
}

/// 同名の項目ごとの決定、またはすべてに同じ決定。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum Decisions {
    All { all: ConflictDecision },
    PerKey(BTreeMap<String, ConflictDecision>),
}

impl Default for Decisions {
    fn default() -> Self {
        Self::PerKey(BTreeMap::new())
    }
}

impl Decisions {
    /// 決定がないキーは置き換えとして扱う（衝突がない場合に呼ばれる）。
    pub fn get(&self, key: &str) -> ConflictDecision {
        match self {
            Self::All { all } => *all,
            Self::PerKey(map) => map.get(key).copied().unwrap_or(ConflictDecision::Replace),
        }
    }
}

/// アーカイブの取り出し速度（DLG-07）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RestoreTier {
    Expedited,
    Standard,
    Bulk,
}

impl RestoreTier {
    pub fn to_sdk(self) -> aws_sdk_s3::types::Tier {
        match self {
            Self::Expedited => aws_sdk_s3::types::Tier::Expedited,
            Self::Standard => aws_sdk_s3::types::Tier::Standard,
            Self::Bulk => aws_sdk_s3::types::Tier::Bulk,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Expedited => "expedited",
            Self::Standard => "standard",
            Self::Bulk => "bulk",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_is_tagged_by_type() {
        let entry = Entry::Folder {
            key: "a/".into(),
            name: "a".into(),
            last_modified: None,
            deleted: false,
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["type"], "folder");
        assert_eq!(json["lastModified"], serde_json::Value::Null);
    }

    #[test]
    fn restore_state_is_tagged_by_state() {
        let json = serde_json::to_value(RestoreState::Restored {
            expiry: "2026-10-01T00:00:00Z".into(),
        })
        .unwrap();
        assert_eq!(json["state"], "restored");
        assert_eq!(json["expiry"], "2026-10-01T00:00:00Z");
        let json = serde_json::to_value(RestoreState::NotArchived).unwrap();
        assert_eq!(json["state"], "notArchived");
    }

    #[test]
    fn decisions_accept_both_shapes() {
        let all: Decisions = serde_json::from_str(r#"{"all":"skip"}"#).unwrap();
        assert_eq!(all.get("x"), ConflictDecision::Skip);
        let per: Decisions = serde_json::from_str(r#"{"a.txt":"keepBoth"}"#).unwrap();
        assert_eq!(per.get("a.txt"), ConflictDecision::KeepBoth);
        assert_eq!(per.get("b.txt"), ConflictDecision::Replace);
    }

    #[test]
    fn batch_event_uses_event_and_data() {
        let json = serde_json::to_value(BatchEvent::Progress(BatchProgress {
            job_id: "j".into(),
            done: 1,
            total: Some(3),
        }))
        .unwrap();
        assert_eq!(json["event"], "progress");
        assert_eq!(json["data"]["jobId"], "j");
    }
}

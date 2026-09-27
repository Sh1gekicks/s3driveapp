use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::common::{FileKind, JobId, StorageClass, Timestamp};
use super::object::Entry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum SizeFilter {
    #[serde(rename = "lt1")]
    Lt1,
    #[serde(rename = "1to100")]
    OneTo100,
    #[serde(rename = "gt100")]
    Gt100,
}

impl SizeFilter {
    /// 10 進（1 MB = 1,000,000 バイト）の範囲 [min, max)（04 §10.2）。
    pub fn range(self) -> (Option<u64>, Option<u64>) {
        const MB: u64 = 1_000_000;
        match self {
            Self::Lt1 => (None, Some(MB)),
            Self::OneTo100 => (Some(MB), Some(100 * MB)),
            Self::Gt100 => (Some(100 * MB), None),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum DateFilter {
    #[serde(rename = "7d")]
    Days7,
    #[serde(rename = "30d")]
    Days30,
    #[serde(rename = "year")]
    Year,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SortKey {
    Name,
    Modified,
    Size,
    StorageClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Sort {
    pub key: SortKey,
    /// 1 は昇順、-1 は降順。
    #[ts(type = "1 | -1")]
    pub dir: i8,
}

impl Default for Sort {
    fn default() -> Self {
        Self {
            key: SortKey::Name,
            dir: 1,
        }
    }
}

/// 検索条件（04 §10.2）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchQuery {
    pub text: String,
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<FileKind>,
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<String>,
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<SizeFilter>,
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<DateFilter>,
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_class: Option<StorageClass>,
    #[serde(default)]
    pub sort: Sort,
    #[serde(default)]
    pub offset: u32,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    1000
}

impl SearchQuery {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: None,
            ext: None,
            size: None,
            date: None,
            storage_class: None,
            sort: Sort::default(),
            offset: 0,
            limit: default_limit(),
        }
    }

    pub fn has_filters(&self) -> bool {
        self.kind.is_some()
            || self.ext.as_deref().is_some_and(|e| !e.trim().is_empty())
            || self.size.is_some()
            || self.date.is_some()
            || self.storage_class.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum IndexState {
    None,
    Building,
    Ready,
    Stale,
}

/// 検索インデックスの状態。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IndexStatus {
    pub state: IndexState,
    pub object_count: u64,
    pub last_scan_at: Option<Timestamp>,
    pub size_bytes: u64,
    /// 作成中の走査済み件数。
    pub progress: Option<u64>,
}

impl IndexStatus {
    pub fn none() -> Self {
        Self {
            state: IndexState::None,
            object_count: 0,
            last_scan_at: None,
            size_bytes: 0,
            progress: None,
        }
    }
}

/// 検索結果の項目（親フォルダのパスを併記する）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchEntry {
    pub entry: Entry,
    pub parent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchResult {
    pub entries: Vec<SearchEntry>,
    pub total: u64,
    pub index: IndexStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
#[ts(export)]
pub enum IndexEvent {
    #[serde(rename_all = "camelCase")]
    Progress { job_id: JobId, scanned: u64 },
    #[serde(rename_all = "camelCase")]
    Finished { job_id: JobId, status: IndexStatus },
    #[serde(rename_all = "camelCase")]
    Failed {
        job_id: JobId,
        error: crate::error::AppError,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_query_uses_design_literals() {
        let q: SearchQuery = serde_json::from_str(
            r#"{"text":"rep","size":"1to100","date":"7d","sort":{"key":"modified","dir":-1},"offset":0,"limit":50}"#,
        )
        .unwrap();
        assert_eq!(q.size, Some(SizeFilter::OneTo100));
        assert_eq!(q.date, Some(DateFilter::Days7));
        assert_eq!(q.sort.dir, -1);
        assert!(q.has_filters());
        assert!(!SearchQuery::text("a").has_filters());
    }

    #[test]
    fn size_ranges_are_decimal() {
        assert_eq!(SizeFilter::Lt1.range(), (None, Some(1_000_000)));
        assert_eq!(SizeFilter::Gt100.range(), (Some(100_000_000), None));
    }
}

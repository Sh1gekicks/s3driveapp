use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::common::{StorageClass, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MetricsSource {
    Cloudwatch,
    Index,
    None,
}

/// クラス別の容量とオブジェクト数（04 §12.2）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StorageMetrics {
    pub source: MetricsSource,
    pub as_of: Option<Timestamp>,
    pub total_bytes: u64,
    pub object_count: Option<u64>,
    pub by_class: BTreeMap<StorageClass, u64>,
}

impl StorageMetrics {
    pub fn none() -> Self {
        Self {
            source: MetricsSource::None,
            as_of: None,
            total_bytes: 0,
            object_count: None,
            by_class: BTreeMap::new(),
        }
    }
}

/// コストの対象範囲（Q4）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum CostScope {
    Tag { key: String, value: String },
    AccountRegion { region: String },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CostBreakdown {
    pub storage: f64,
    pub requests: f64,
    pub transfer: f64,
    pub retrieval: f64,
    pub other: f64,
}

/// Cost Explorer から取得したコスト（04 §13）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CostSummary {
    pub scope: CostScope,
    /// 対象月（例: 2026-09）。
    pub month: String,
    pub month_to_date: f64,
    pub prev_month_same_period: Option<f64>,
    pub breakdown: CostBreakdown,
    /// 当月の日数分。未来日は null。
    pub daily: Vec<Option<f64>>,
    pub forecast_month_end: Option<f64>,
    #[ts(type = "'USD'")]
    pub currency: String,
    pub fetched_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PriceSource {
    Api,
    Bundled,
}

/// GB・月あたりの単価（04 §13.3）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PriceTable {
    pub region: String,
    pub per_gb_month: BTreeMap<StorageClass, f64>,
    pub source: PriceSource,
    pub as_of: Timestamp,
}

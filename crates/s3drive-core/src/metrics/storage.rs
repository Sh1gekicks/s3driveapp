//! 利用容量（CloudWatch の S3 日次メトリクス。04 §12.2）。

use std::collections::BTreeMap;

use aws_sdk_cloudwatch::types::{Dimension, Metric, MetricDataQuery, MetricStat};
use chrono::Duration;

use crate::Core;
use crate::aws::error_map::{self, Ctx};
use crate::error::CoreResult;
use crate::model::{MetricsSource, StorageClass, StorageMetrics};
use crate::store::metrics_cache;
use crate::util::time;

const CACHE_KIND: &str = "storage";

/// CloudWatch の `StorageType` から DS の 7 クラスへの集計（04 §12.2）。
pub const STORAGE_TYPES: &[(&str, StorageClass)] = &[
    ("StandardStorage", StorageClass::Standard),
    ("GlacierS3ObjectOverhead", StorageClass::Standard),
    ("DeepArchiveS3ObjectOverhead", StorageClass::Standard),
    ("IntAAS3ObjectOverhead", StorageClass::Standard),
    ("IntDAAS3ObjectOverhead", StorageClass::Standard),
    (
        "IntelligentTieringFAStorage",
        StorageClass::IntelligentTiering,
    ),
    (
        "IntelligentTieringIAStorage",
        StorageClass::IntelligentTiering,
    ),
    (
        "IntelligentTieringAIAStorage",
        StorageClass::IntelligentTiering,
    ),
    (
        "IntelligentTieringAAStorage",
        StorageClass::IntelligentTiering,
    ),
    (
        "IntelligentTieringDAAStorage",
        StorageClass::IntelligentTiering,
    ),
    ("IntAAObjectOverhead", StorageClass::IntelligentTiering),
    ("IntDAAObjectOverhead", StorageClass::IntelligentTiering),
    ("StandardIAStorage", StorageClass::StandardIa),
    ("StandardIASizeOverhead", StorageClass::StandardIa),
    ("OneZoneIAStorage", StorageClass::OnezoneIa),
    ("OneZoneIASizeOverhead", StorageClass::OnezoneIa),
    ("GlacierInstantRetrievalStorage", StorageClass::GlacierIr),
    ("GlacierIRSizeOverhead", StorageClass::GlacierIr),
    ("GlacierStorage", StorageClass::Glacier),
    ("GlacierObjectOverhead", StorageClass::Glacier),
    ("GlacierStagingStorage", StorageClass::Glacier),
    ("DeepArchiveStorage", StorageClass::DeepArchive),
    ("DeepArchiveObjectOverhead", StorageClass::DeepArchive),
    ("DeepArchiveStagingStorage", StorageClass::DeepArchive),
    ("ReducedRedundancyStorage", StorageClass::Other),
];

/// CloudWatch の結果（StorageType ごとの最新値）をクラス別に集計する。
pub fn aggregate(values: &[(usize, f64)]) -> BTreeMap<StorageClass, u64> {
    let mut by_class = BTreeMap::new();
    for (index, value) in values {
        if let Some((_, class)) = STORAGE_TYPES.get(*index) {
            *by_class.entry(*class).or_insert(0u64) += value.max(0.0).round() as u64;
        }
    }
    by_class.retain(|_, v| *v > 0);
    by_class
}

impl Core {
    /// クラス別の容量とオブジェクト数（1 時間キャッシュ）。取得できなければインデックスから集計する。
    pub async fn metrics_storage(
        &self,
        connection_id: &str,
        force: bool,
    ) -> CoreResult<StorageMetrics> {
        let ctx = self.ctx(connection_id).await?;
        if !force
            && let Some(hit) =
                metrics_cache::get::<StorageMetrics>(&self.0.db, connection_id, CACHE_KIND).await?
            && !hit.expired
        {
            return Ok(hit.value);
        }
        let metrics = match fetch_cloudwatch(&ctx.clients.cloudwatch, &ctx.bucket).await {
            Ok(Some(m)) => m,
            Ok(None) => self.metrics_from_index(connection_id).await?,
            Err(e) => {
                log::info!(
                    "CloudWatch のメトリクスを取得できないため、インデックスから集計します: {e}"
                );
                self.metrics_from_index(connection_id).await?
            }
        };
        if metrics.source == MetricsSource::Cloudwatch {
            metrics_cache::put(
                &self.0.db,
                connection_id,
                CACHE_KIND,
                &metrics,
                Some(Duration::hours(1)),
            )
            .await?;
        }
        Ok(metrics)
    }

    async fn metrics_from_index(&self, connection_id: &str) -> CoreResult<StorageMetrics> {
        Ok(
            match crate::search::class_totals(&self.0.db, connection_id).await? {
                Some((totals, count)) => StorageMetrics {
                    source: MetricsSource::Index,
                    as_of: None,
                    total_bytes: totals.values().sum(),
                    object_count: Some(count),
                    by_class: totals.into_iter().filter(|(_, v)| *v > 0).collect(),
                },
                None => StorageMetrics::none(),
            },
        )
    }
}

/// `GetMetricData` を 1 回呼ぶ。データポイントがなければ `None`（作成直後のバケット）。
async fn fetch_cloudwatch(
    cw: &aws_sdk_cloudwatch::Client,
    bucket: &str,
) -> CoreResult<Option<StorageMetrics>> {
    let bucket_dim = Dimension::builder()
        .name("BucketName")
        .value(bucket)
        .build();
    let query = |id: String, metric: &str, storage_type: &str| {
        MetricDataQuery::builder()
            .id(id)
            .metric_stat(
                MetricStat::builder()
                    .metric(
                        Metric::builder()
                            .namespace("AWS/S3")
                            .metric_name(metric)
                            .dimensions(bucket_dim.clone())
                            .dimensions(
                                Dimension::builder()
                                    .name("StorageType")
                                    .value(storage_type)
                                    .build(),
                            )
                            .build(),
                    )
                    .period(86_400)
                    .stat("Average")
                    .build(),
            )
            .return_data(true)
            .build()
    };
    let mut queries: Vec<MetricDataQuery> = STORAGE_TYPES
        .iter()
        .enumerate()
        .map(|(i, (st, _))| query(format!("s{i}"), "BucketSizeBytes", st))
        .collect();
    queries.push(query(
        "objects".into(),
        "NumberOfObjects",
        "AllStorageTypes",
    ));

    let end = time::now();
    let start = end - Duration::days(3);
    let out = cw
        .get_metric_data()
        .set_metric_data_queries(Some(queries))
        .start_time(aws_smithy_types::DateTime::from_secs(start.timestamp()))
        .end_time(aws_smithy_types::DateTime::from_secs(end.timestamp()))
        .send()
        .await
        .map_err(|e| error_map::classify(&e, Ctx::Metrics))?;

    let mut values = Vec::new();
    let mut objects = None;
    let mut as_of: Option<aws_smithy_types::DateTime> = None;
    for r in out.metric_data_results() {
        // 既定は新しい順。最新のデータポイントを使う
        let latest = r
            .timestamps()
            .iter()
            .zip(r.values())
            .max_by_key(|(t, _)| t.secs());
        let Some((t, v)) = latest else { continue };
        if as_of.is_none_or(|a| t.secs() > a.secs()) {
            as_of = Some(*t);
        }
        match r.id() {
            Some("objects") => objects = Some(v.max(0.0).round() as u64),
            Some(id) => {
                if let Some(i) = id.strip_prefix('s').and_then(|n| n.parse::<usize>().ok()) {
                    values.push((i, *v));
                }
            }
            None => {}
        }
    }
    if values.is_empty() && objects.is_none() {
        return Ok(None);
    }
    let by_class = aggregate(&values);
    Ok(Some(StorageMetrics {
        source: MetricsSource::Cloudwatch,
        as_of: as_of.as_ref().map(time::aws_to_rfc3339),
        total_bytes: by_class.values().sum(),
        object_count: objects,
        by_class,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_cloudwatch::operation::get_metric_data::GetMetricDataOutput;
    use aws_sdk_cloudwatch::types::MetricDataResult;
    use aws_smithy_mocks::{mock, mock_client};

    fn index_of(storage_type: &str) -> usize {
        STORAGE_TYPES
            .iter()
            .position(|(s, _)| *s == storage_type)
            .unwrap()
    }

    #[test]
    fn aggregates_storage_types_into_classes() {
        let values = vec![
            (index_of("StandardStorage"), 100.0),
            (index_of("GlacierS3ObjectOverhead"), 8.0),
            (index_of("GlacierStorage"), 1000.0),
            (index_of("GlacierObjectOverhead"), 32.0),
            (index_of("IntelligentTieringFAStorage"), 5.0),
            (index_of("IntelligentTieringAAStorage"), 7.0),
            (index_of("ReducedRedundancyStorage"), 1.0),
        ];
        let by_class = aggregate(&values);
        assert_eq!(by_class[&StorageClass::Standard], 108);
        assert_eq!(by_class[&StorageClass::Glacier], 1032);
        assert_eq!(by_class[&StorageClass::IntelligentTiering], 12);
        assert_eq!(by_class[&StorageClass::Other], 1);
    }

    #[tokio::test]
    async fn reads_the_latest_datapoints() {
        let rule = mock!(aws_sdk_cloudwatch::Client::get_metric_data).then_output(|| {
            GetMetricDataOutput::builder()
                .metric_data_results(
                    MetricDataResult::builder()
                        .id(format!("s{}", index_of("StandardStorage")))
                        .timestamps(aws_smithy_types::DateTime::from_secs(2_000))
                        .values(500.0)
                        .timestamps(aws_smithy_types::DateTime::from_secs(1_000))
                        .values(400.0)
                        .build(),
                )
                .metric_data_results(
                    MetricDataResult::builder()
                        .id("objects")
                        .timestamps(aws_smithy_types::DateTime::from_secs(2_000))
                        .values(12.0)
                        .build(),
                )
                .build()
        });
        let cw = mock_client!(aws_sdk_cloudwatch, [&rule]);
        let m = fetch_cloudwatch(&cw, "b").await.unwrap().unwrap();
        assert_eq!(m.source, MetricsSource::Cloudwatch);
        assert_eq!(m.total_bytes, 500);
        assert_eq!(m.object_count, Some(12));
        assert_eq!(m.as_of.as_deref(), Some("1970-01-01T00:33:20Z"));
    }

    #[tokio::test]
    async fn returns_none_without_datapoints() {
        let rule = mock!(aws_sdk_cloudwatch::Client::get_metric_data)
            .then_output(|| GetMetricDataOutput::builder().build());
        let cw = mock_client!(aws_sdk_cloudwatch, [&rule]);
        assert!(fetch_cloudwatch(&cw, "b").await.unwrap().is_none());
    }
}

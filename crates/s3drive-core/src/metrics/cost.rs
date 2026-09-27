//! コスト（Cost Explorer。04 §13）。
//!
//! Cost Explorer の API は 1 リクエストあたり $0.01 かかるため、**ユーザーが「更新」を押したときだけ**
//! 問い合わせる（04 §13.4）。結果は SQLite に保存し、次に更新するまで表示し続ける。

use aws_sdk_costexplorer::types::{
    DateInterval, Dimension, DimensionValues, Expression, Granularity, GroupDefinition,
    GroupDefinitionType, Metric, TagValues,
};
use chrono::{Datelike, Duration, NaiveDate};

use crate::Core;
use crate::aws::error_map::{self, Ctx};
use crate::error::{CoreError, CoreResult, ErrorCode};
use crate::model::{CostBreakdown, CostScope, CostSummary, CostTag};
use crate::store::metrics_cache;
use crate::util::time;

const CACHE_KIND: &str = "cost";
const SERVICE_NAME: &str = "Amazon Simple Storage Service";
const METRIC: &str = "UnblendedCost";

/// 問い合わせる期間（終了日は含まない）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Periods {
    pub month_start: NaiveDate,
    /// 当月累計の終了（明日。今日の分を含める）。
    pub mtd_end: NaiveDate,
    pub prev_start: NaiveDate,
    /// 前月の同じ日数分の終了。
    pub prev_end: NaiveDate,
    pub forecast_start: NaiveDate,
    pub forecast_end: NaiveDate,
    pub days_in_month: u32,
}

/// 当月・前月同期間・予測の期間（UTC の日付で計算する）。
pub fn periods(today: NaiveDate) -> Periods {
    let month_start = time::first_of_month(today);
    let next_month = time::first_of_next_month(today);
    let mtd_end = (today + Duration::days(1)).min(next_month);
    let elapsed = (mtd_end - month_start).num_days();
    let prev_start = time::first_of_prev_month(today);
    let prev_end = (prev_start + Duration::days(elapsed)).min(month_start);
    Periods {
        month_start,
        mtd_end,
        prev_start,
        prev_end,
        forecast_start: today,
        forecast_end: next_month,
        days_in_month: time::days_in_month(today),
    }
}

/// 使用タイプの分類（04 §13.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageCategory {
    Storage,
    Requests,
    Transfer,
    Retrieval,
    Other,
}

pub fn classify_usage_type(usage_type: &str) -> UsageCategory {
    // リージョン接頭辞（例: APN1-）は分類に影響しないため、含まれる語だけで判定する
    if usage_type.contains("TimedStorage") {
        UsageCategory::Storage
    } else if usage_type.contains("Requests") {
        UsageCategory::Requests
    } else if usage_type.contains("DataTransfer") || usage_type.contains("Out-Bytes") {
        UsageCategory::Transfer
    } else if usage_type.contains("Retrieval") || usage_type.contains("Restore") {
        UsageCategory::Retrieval
    } else {
        UsageCategory::Other
    }
}

fn ymd(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn interval(start: NaiveDate, end: NaiveDate) -> CoreResult<DateInterval> {
    DateInterval::builder()
        .start(ymd(start))
        .end(ymd(end))
        .build()
        .map_err(CoreError::internal)
}

/// 共通のフィルタ: S3 かつバケットのリージョン（コスト配分タグがあればそれも）。
pub fn filter(region: &str, tag: Option<&CostTag>) -> Expression {
    let dim = |key: Dimension, value: &str| {
        Expression::builder()
            .dimensions(DimensionValues::builder().key(key).values(value).build())
            .build()
    };
    let mut and = vec![
        dim(Dimension::Service, SERVICE_NAME),
        dim(Dimension::Region, region),
    ];
    if let Some(t) = tag {
        and.push(
            Expression::builder()
                .tags(TagValues::builder().key(&t.key).values(&t.value).build())
                .build(),
        );
    }
    Expression::builder().set_and(Some(and)).build()
}

fn amount(
    map: Option<&std::collections::HashMap<String, aws_sdk_costexplorer::types::MetricValue>>,
) -> f64 {
    map.and_then(|m| m.get(METRIC))
        .and_then(|v| v.amount())
        .and_then(|a| a.parse::<f64>().ok())
        .unwrap_or(0.0)
}

impl Core {
    /// 保存済みのコスト（Cost Explorer には問い合わせない）。
    pub async fn cost_summary(&self, connection_id: &str) -> CoreResult<Option<CostSummary>> {
        self.ctx(connection_id).await?;
        Ok(
            metrics_cache::get::<CostSummary>(&self.0.db, connection_id, CACHE_KIND)
                .await?
                .map(|c| c.value),
        )
    }

    /// Cost Explorer に問い合わせて保存する。ダッシュボードの「更新」「取得」からのみ呼ぶ。
    pub async fn cost_refresh(&self, connection_id: &str) -> CoreResult<CostSummary> {
        if !self.0.settings.read(|f| f.settings.cost.use_cost_explorer) {
            return Err(CoreError::with_message(
                ErrorCode::CostUnavailable,
                "Cost Explorer は設定で無効になっています",
            ));
        }
        let ctx = self.ctx(connection_id).await?;
        let summary = fetch(
            &ctx.clients.cost,
            &ctx.region,
            ctx.record.cost_tag.as_ref(),
            time::now().date_naive(),
        )
        .await?;
        metrics_cache::put(&self.0.db, connection_id, CACHE_KIND, &summary, None).await?;
        Ok(summary)
    }
}

/// Cost Explorer に 4 回問い合わせる（当月の使用タイプ別、前月同期間、当月の日別、予測）。
pub async fn fetch(
    ce: &aws_sdk_costexplorer::Client,
    region: &str,
    tag: Option<&CostTag>,
    today: NaiveDate,
) -> CoreResult<CostSummary> {
    let p = periods(today);
    let f = filter(region, tag);

    // 1. 当月累計（使用タイプ別）
    let mut breakdown = CostBreakdown::default();
    let mut month_to_date = 0.0;
    let mut token = None;
    loop {
        let out = ce
            .get_cost_and_usage()
            .time_period(interval(p.month_start, p.mtd_end)?)
            .granularity(Granularity::Monthly)
            .metrics(METRIC)
            .filter(f.clone())
            .group_by(
                GroupDefinition::builder()
                    .r#type(GroupDefinitionType::Dimension)
                    .key("USAGE_TYPE")
                    .build(),
            )
            .set_next_page_token(token)
            .send()
            .await
            .map_err(|e| error_map::classify(&e, Ctx::Cost))?;
        for result in out.results_by_time() {
            for g in result.groups() {
                let value = amount(g.metrics());
                month_to_date += value;
                let usage = g.keys().first().map(String::as_str).unwrap_or("");
                match classify_usage_type(usage) {
                    UsageCategory::Storage => breakdown.storage += value,
                    UsageCategory::Requests => breakdown.requests += value,
                    UsageCategory::Transfer => breakdown.transfer += value,
                    UsageCategory::Retrieval => breakdown.retrieval += value,
                    UsageCategory::Other => breakdown.other += value,
                }
            }
        }
        token = out.next_page_token().map(str::to_string);
        if token.is_none() {
            break;
        }
    }

    // 2. 前月の同じ日数分
    let prev_month_same_period = if p.prev_end > p.prev_start {
        let out = ce
            .get_cost_and_usage()
            .time_period(interval(p.prev_start, p.prev_end)?)
            .granularity(Granularity::Monthly)
            .metrics(METRIC)
            .filter(f.clone())
            .send()
            .await
            .map_err(|e| error_map::classify(&e, Ctx::Cost))?;
        Some(
            out.results_by_time()
                .iter()
                .map(|r| amount(r.total()))
                .sum(),
        )
    } else {
        None
    };

    // 3. 当月の日別
    let out = ce
        .get_cost_and_usage()
        .time_period(interval(p.month_start, p.mtd_end)?)
        .granularity(Granularity::Daily)
        .metrics(METRIC)
        .filter(f.clone())
        .send()
        .await
        .map_err(|e| error_map::classify(&e, Ctx::Cost))?;
    let mut daily: Vec<Option<f64>> = vec![None; p.days_in_month as usize];
    for r in out.results_by_time() {
        let day = r
            .time_period()
            .and_then(|t| NaiveDate::parse_from_str(t.start(), "%Y-%m-%d").ok());
        if let Some(d) = day
            && d.month() == today.month()
            && let Some(slot) = daily.get_mut(d.day0() as usize)
        {
            *slot = Some(amount(r.total()));
        }
    }
    for (i, slot) in daily.iter_mut().enumerate() {
        if slot.is_none() && (i as u32) < today.day() {
            *slot = Some(0.0);
        }
    }

    // 4. 月末予測（十分な履歴がないと予測できないため、失敗は「予測なし」とする）
    let forecast_month_end = match ce
        .get_cost_forecast()
        .time_period(interval(p.forecast_start, p.forecast_end)?)
        .granularity(Granularity::Monthly)
        .metric(Metric::UnblendedCost)
        .filter(f)
        .send()
        .await
    {
        Ok(out) => out
            .total()
            .and_then(|v| v.amount())
            .and_then(|a| a.parse::<f64>().ok())
            .map(|v| v + month_to_date),
        Err(e) => {
            log::info!(
                "コストの予測を取得できません: {}",
                error_map::classify(&e, Ctx::Cost)
            );
            None
        }
    };

    Ok(CostSummary {
        scope: match tag {
            Some(t) => CostScope::Tag {
                key: t.key.clone(),
                value: t.value.clone(),
            },
            None => CostScope::AccountRegion {
                region: region.to_string(),
            },
        },
        month: today.format("%Y-%m").to_string(),
        month_to_date,
        prev_month_same_period,
        breakdown,
        daily,
        forecast_month_end,
        currency: "USD".to_string(),
        fetched_at: time::now_rfc3339(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_costexplorer::operation::get_cost_and_usage::GetCostAndUsageOutput;
    use aws_sdk_costexplorer::operation::get_cost_forecast::{
        GetCostForecastError, GetCostForecastOutput,
    };
    use aws_sdk_costexplorer::types::{Group, MetricValue, ResultByTime};
    use aws_smithy_mocks::{RuleMode, mock, mock_client};

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn computes_periods_mid_month() {
        let p = periods(d(2026, 9, 27));
        assert_eq!(p.month_start, d(2026, 9, 1));
        assert_eq!(p.mtd_end, d(2026, 9, 28));
        assert_eq!(p.prev_start, d(2026, 8, 1));
        assert_eq!(p.prev_end, d(2026, 8, 28));
        assert_eq!(p.forecast_end, d(2026, 10, 1));
        assert_eq!(p.days_in_month, 30);
    }

    #[test]
    fn caps_previous_period_at_month_end() {
        // 3 月 31 日の前月同期間は 2 月末まで
        let p = periods(d(2026, 3, 31));
        assert_eq!(p.mtd_end, d(2026, 4, 1));
        assert_eq!(p.prev_start, d(2026, 2, 1));
        assert_eq!(p.prev_end, d(2026, 3, 1));
        // 1 月は前年の 12 月と比べる
        let p = periods(d(2027, 1, 1));
        assert_eq!(p.prev_start, d(2026, 12, 1));
        assert_eq!(p.prev_end, d(2026, 12, 2));
    }

    #[test]
    fn classifies_usage_types() {
        assert_eq!(
            classify_usage_type("APN1-TimedStorage-SIA-ByteHrs"),
            UsageCategory::Storage
        );
        assert_eq!(
            classify_usage_type("APN1-Requests-Tier1"),
            UsageCategory::Requests
        );
        assert_eq!(
            classify_usage_type("APN1-DataTransfer-Out-Bytes"),
            UsageCategory::Transfer
        );
        assert_eq!(
            classify_usage_type("APN1-Retrieval-SIA"),
            UsageCategory::Retrieval
        );
        assert_eq!(
            classify_usage_type("APN1-Restore-Bulk"),
            UsageCategory::Retrieval
        );
        assert_eq!(
            classify_usage_type("APN1-EarlyDelete-SIA"),
            UsageCategory::Other
        );
    }

    #[test]
    fn builds_filters_with_and_without_tags() {
        let f = filter("ap-northeast-1", None);
        assert_eq!(f.and().len(), 2);
        let tag = CostTag {
            key: "app".into(),
            value: "s3drive".into(),
        };
        let f = filter("ap-northeast-1", Some(&tag));
        assert_eq!(f.and().len(), 3);
        assert_eq!(f.and()[2].tags().unwrap().key(), Some("app"));
    }

    fn money(v: &str) -> MetricValue {
        MetricValue::builder().amount(v).unit("USD").build()
    }

    #[tokio::test]
    async fn fetches_and_summarizes_costs() {
        let grouped = mock!(aws_sdk_costexplorer::Client::get_cost_and_usage)
            .match_requests(|r| !r.group_by().is_empty())
            .then_output(|| {
                GetCostAndUsageOutput::builder()
                    .results_by_time(
                        ResultByTime::builder()
                            .groups(
                                Group::builder()
                                    .keys("APN1-TimedStorage-ByteHrs")
                                    .metrics(METRIC, money("5.00"))
                                    .build(),
                            )
                            .groups(
                                Group::builder()
                                    .keys("APN1-Requests-Tier1")
                                    .metrics(METRIC, money("0.42"))
                                    .build(),
                            )
                            .groups(
                                Group::builder()
                                    .keys("APN1-DataTransfer-Out-Bytes")
                                    .metrics(METRIC, money("1.14"))
                                    .build(),
                            )
                            .build(),
                    )
                    .build()
            });
        let prev = mock!(aws_sdk_costexplorer::Client::get_cost_and_usage)
            .match_requests(|r| r.granularity() == Some(&Granularity::Monthly))
            .then_output(|| {
                GetCostAndUsageOutput::builder()
                    .results_by_time(ResultByTime::builder().total(METRIC, money("6.00")).build())
                    .build()
            });
        let daily = mock!(aws_sdk_costexplorer::Client::get_cost_and_usage)
            .match_requests(|r| r.granularity() == Some(&Granularity::Daily))
            .then_output(|| {
                GetCostAndUsageOutput::builder()
                    .results_by_time(
                        ResultByTime::builder()
                            .time_period(
                                DateInterval::builder()
                                    .start("2026-09-01")
                                    .end("2026-09-02")
                                    .build()
                                    .unwrap(),
                            )
                            .total(METRIC, money("0.25"))
                            .build(),
                    )
                    .build()
            });
        let forecast = mock!(aws_sdk_costexplorer::Client::get_cost_forecast).then_output(|| {
            GetCostForecastOutput::builder()
                .total(money("1.00"))
                .build()
        });
        let ce = mock_client!(
            aws_sdk_costexplorer,
            RuleMode::MatchAny,
            [&grouped, &prev, &daily, &forecast]
        );
        let s = fetch(&ce, "ap-northeast-1", None, d(2026, 9, 3))
            .await
            .unwrap();
        assert!((s.month_to_date - 6.56).abs() < 1e-9);
        assert!((s.breakdown.storage - 5.0).abs() < 1e-9);
        assert!((s.breakdown.requests - 0.42).abs() < 1e-9);
        assert_eq!(s.prev_month_same_period, Some(6.0));
        assert_eq!(s.daily.len(), 30);
        assert_eq!(s.daily[0], Some(0.25));
        assert_eq!(s.daily[1], Some(0.0));
        assert_eq!(s.daily[3], None);
        assert!((s.forecast_month_end.unwrap() - 7.56).abs() < 1e-9);
        assert_eq!(s.month, "2026-09");
        assert!(matches!(s.scope, CostScope::AccountRegion { .. }));
        assert_eq!(forecast.num_calls(), 1);
    }

    #[tokio::test]
    async fn missing_forecast_is_not_an_error() {
        let usage = mock!(aws_sdk_costexplorer::Client::get_cost_and_usage)
            .then_output(|| GetCostAndUsageOutput::builder().build());
        let forecast = mock!(aws_sdk_costexplorer::Client::get_cost_forecast).then_error(|| {
            GetCostForecastError::DataUnavailableException(
                aws_sdk_costexplorer::types::error::DataUnavailableException::builder().build(),
            )
        });
        let ce = mock_client!(
            aws_sdk_costexplorer,
            RuleMode::MatchAny,
            [&usage, &forecast]
        );
        let s = fetch(&ce, "ap-northeast-1", None, d(2026, 9, 3))
            .await
            .unwrap();
        assert_eq!(s.forecast_month_end, None);
    }
}

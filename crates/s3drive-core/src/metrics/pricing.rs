//! ストレージの単価（Price List API。04 §13.3）。取得できない場合は同梱の単価表を使う。

use std::collections::BTreeMap;

use aws_sdk_pricing::types::{Filter, FilterType};
use chrono::Duration;

use crate::Core;
use crate::aws::error_map::{self, Ctx};
use crate::error::{CoreError, CoreResult};
use crate::model::{PriceSource, PriceTable, StorageClass};
use crate::store::metrics_cache;
use crate::util::time;

/// 同梱の単価表の基準日。リリース前に公開料金と照合して更新する。
pub const BUNDLED_AS_OF: &str = "2026-09-27T00:00:00Z";

/// 同梱の単価表（USD / GB・月、最初の段階）。
fn bundled(region: &str) -> Option<BTreeMap<StorageClass, f64>> {
    use StorageClass::*;
    let values: &[(StorageClass, f64)] = match region {
        "ap-northeast-1" => &[
            (Standard, 0.025),
            (IntelligentTiering, 0.025),
            (StandardIa, 0.0138),
            (OnezoneIa, 0.011),
            (GlacierIr, 0.005),
            (Glacier, 0.0045),
            (DeepArchive, 0.002),
        ],
        "us-east-1" => &[
            (Standard, 0.023),
            (IntelligentTiering, 0.023),
            (StandardIa, 0.0125),
            (OnezoneIa, 0.01),
            (GlacierIr, 0.004),
            (Glacier, 0.0036),
            (DeepArchive, 0.00099),
        ],
        _ => return None,
    };
    Some(values.iter().copied().collect())
}

/// Price List の `volumeType` 属性とクラスの対応。
pub fn class_for_volume_type(volume_type: &str) -> Option<StorageClass> {
    Some(match volume_type {
        "Standard" => StorageClass::Standard,
        "Intelligent-Tiering Frequent Access" => StorageClass::IntelligentTiering,
        "Standard - Infrequent Access" => StorageClass::StandardIa,
        "One Zone - Infrequent Access" => StorageClass::OnezoneIa,
        "Glacier Instant Retrieval" => StorageClass::GlacierIr,
        "Amazon Glacier" | "Glacier Flexible Retrieval" => StorageClass::Glacier,
        "Glacier Deep Archive" => StorageClass::DeepArchive,
        _ => return None,
    })
}

/// Price List の 1 件（JSON 文字列）から、クラスと最初の段階の単価を取り出す。
pub fn parse_price_item(json: &str) -> Option<(StorageClass, f64)> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let class = class_for_volume_type(v["product"]["attributes"]["volumeType"].as_str()?)?;
    let on_demand = v["terms"]["OnDemand"].as_object()?;
    for term in on_demand.values() {
        for dim in term["priceDimensions"].as_object()?.values() {
            let begin = dim["beginRange"].as_str().unwrap_or("0");
            if begin == "0" && dim["unit"].as_str().is_some_and(|u| u.starts_with("GB")) {
                return dim["pricePerUnit"]["USD"]
                    .as_str()?
                    .parse::<f64>()
                    .ok()
                    .map(|p| (class, p));
            }
        }
    }
    None
}

async fn fetch(
    pricing: &aws_sdk_pricing::Client,
    region: &str,
) -> CoreResult<BTreeMap<StorageClass, f64>> {
    let filter = |field: &str, value: &str| {
        Filter::builder()
            .r#type(FilterType::TermMatch)
            .field(field)
            .value(value)
            .build()
            .map_err(CoreError::internal)
    };
    let mut prices = BTreeMap::new();
    let mut token = None;
    for _ in 0..20 {
        let out = pricing
            .get_products()
            .service_code("AmazonS3")
            .filters(filter("regionCode", region)?)
            .filters(filter("productFamily", "Storage")?)
            .set_next_token(token)
            .send()
            .await
            .map_err(|e| error_map::classify(&e, Ctx::Op("pricing:GetProducts")))?;
        for item in out.price_list() {
            if let Some((class, price)) = parse_price_item(item) {
                prices.entry(class).or_insert(price);
            }
        }
        token = out.next_token().map(str::to_string);
        if token.is_none() {
            break;
        }
    }
    Ok(prices)
}

impl Core {
    /// 単価（7 日間キャッシュ）。Price List の呼び出しには、サインイン中のアカウントの接続の認証情報を使う。
    pub async fn pricing_get(&self, region: &str) -> CoreResult<PriceTable> {
        let kind = format!("pricing:{region}");
        if let Some(hit) = metrics_cache::get::<PriceTable>(&self.0.db, "", &kind).await?
            && !hit.expired
        {
            return Ok(hit.value);
        }
        let connection = self
            .connections()
            .list()?
            .into_iter()
            .max_by_key(|c| c.region == region);
        let fetched = match connection {
            Some(c) => {
                let ctx = self.ctx(&c.id).await?;
                fetch(&ctx.clients.pricing, region).await
            }
            None => Err(CoreError::internal("no connection")),
        };
        match fetched {
            Ok(prices) if !prices.is_empty() => {
                let table = PriceTable {
                    region: region.to_string(),
                    per_gb_month: prices,
                    source: PriceSource::Api,
                    as_of: time::now_rfc3339(),
                };
                metrics_cache::put(&self.0.db, "", &kind, &table, Some(Duration::days(7))).await?;
                Ok(table)
            }
            other => {
                if let Err(e) = other {
                    log::info!("単価を取得できないため同梱の単価表を使います: {e}");
                }
                Ok(PriceTable {
                    region: region.to_string(),
                    per_gb_month: bundled(region).unwrap_or_default(),
                    source: PriceSource::Bundled,
                    as_of: BUNDLED_AS_OF.to_string(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_price_list_items() {
        let item = r#"{
          "product": { "productFamily": "Storage", "attributes": { "volumeType": "Standard - Infrequent Access", "regionCode": "ap-northeast-1" } },
          "terms": { "OnDemand": { "X.Y": { "priceDimensions": {
            "X.Y.Z": { "unit": "GB-Mo", "beginRange": "0", "endRange": "Inf", "pricePerUnit": { "USD": "0.0138000000" } }
          } } } }
        }"#;
        assert_eq!(
            parse_price_item(item),
            Some((StorageClass::StandardIa, 0.0138))
        );
    }

    #[test]
    fn uses_the_first_tier() {
        let item = r#"{
          "product": { "attributes": { "volumeType": "Standard" } },
          "terms": { "OnDemand": { "T": { "priceDimensions": {
            "b": { "unit": "GB-Mo", "beginRange": "51200", "pricePerUnit": { "USD": "0.024" } },
            "a": { "unit": "GB-Mo", "beginRange": "0", "pricePerUnit": { "USD": "0.025" } }
          } } } }
        }"#;
        assert_eq!(
            parse_price_item(item),
            Some((StorageClass::Standard, 0.025))
        );
    }

    #[test]
    fn ignores_unknown_volume_types() {
        assert_eq!(class_for_volume_type("Reduced Redundancy"), None);
        assert!(bundled("ap-northeast-1").is_some());
        assert!(bundled("xx-1").is_none());
    }
}

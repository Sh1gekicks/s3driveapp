//! 接続ごとの SDK クライアント（01 §6.1）。

pub mod error_map;

use std::time::Duration;

use aws_config::{BehaviorVersion, Region, SdkConfig};
use aws_credential_types::provider::SharedCredentialsProvider;
use aws_sdk_s3::config::{RequestChecksumCalculation, ResponseChecksumValidation};
use aws_smithy_types::retry::RetryConfig;
use aws_smithy_types::timeout::TimeoutConfig;

use crate::credentials::ExpiredTokenRetry;

/// Cost Explorer と Price List のエンドポイントのリージョン。
pub const GLOBAL_REGION: &str = "us-east-1";

/// 共通の SDK 設定を作る。認証情報は明示したプロバイダだけを使う。
pub async fn sdk_config(
    region: &str,
    credentials: SharedCredentialsProvider,
    endpoint_url: Option<&str>,
) -> SdkConfig {
    sdk_config_with(region, credentials, endpoint_url, true).await
}

/// `cache_identity` が偽なら SDK の ID キャッシュを使わない。AssumeRole のプロバイダが自分でキャッシュし、
/// `ExpiredToken` を受け取ったときに取り直せるようにするため（[`crate::credentials::ExpiredTokenRetry`]）。
pub async fn sdk_config_with(
    region: &str,
    credentials: SharedCredentialsProvider,
    endpoint_url: Option<&str>,
    cache_identity: bool,
) -> SdkConfig {
    let mut loader = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(region.to_string()))
        .credentials_provider(credentials)
        .retry_config(RetryConfig::standard().with_max_attempts(3))
        .timeout_config(
            TimeoutConfig::builder()
                .connect_timeout(Duration::from_secs(10))
                .operation_attempt_timeout(Duration::from_secs(30))
                .build(),
        );
    if let Some(url) = endpoint_url {
        loader = loader.endpoint_url(url);
    }
    if !cache_identity {
        loader = loader.identity_cache(aws_config::identity::IdentityCache::no_cache());
    }
    loader.load().await
}

/// 1 つの接続が使うクライアント群。
#[derive(Clone, Debug)]
pub struct Clients {
    /// 単発の API 呼び出し（standard リトライ、試行 30 秒）。
    pub s3: aws_sdk_s3::Client,
    /// 転送用（本文の送受信があるため試行のタイムアウトを設けない）。
    pub s3_transfer: aws_sdk_s3::Client,
    /// 一括操作用（adaptive リトライで SlowDown に追従する）。
    pub s3_bulk: aws_sdk_s3::Client,
    pub cloudwatch: aws_sdk_cloudwatch::Client,
    pub cost: aws_sdk_costexplorer::Client,
    pub pricing: aws_sdk_pricing::Client,
    /// SDK の既定のチェックサム計算を使うか（テスト用エンドポイントでは必要な場合のみにする）。
    pub full_checksums: bool,
}

impl Clients {
    /// `expired_retry` は AssumeRole を使う接続で指定する（`ExpiredToken` で取り直して 1 回だけ再試行する。01 §7.1）。
    pub fn new(
        config: &SdkConfig,
        endpoint_url: Option<&str>,
        expired_retry: Option<ExpiredTokenRetry>,
    ) -> Self {
        let s3_base = || {
            let mut b = aws_sdk_s3::config::Builder::from(config);
            if let Some(r) = &expired_retry {
                b = b.retry_classifier(r.clone());
            }
            if endpoint_url.is_some() {
                // テスト用エンドポイント（moto）はパス形式にし、必要な場合だけチェックサムを計算する（09 §2.2）
                b = b
                    .force_path_style(true)
                    .request_checksum_calculation(RequestChecksumCalculation::WhenRequired)
                    .response_checksum_validation(ResponseChecksumValidation::WhenRequired);
            }
            b
        };
        let s3 = aws_sdk_s3::Client::from_conf(s3_base().build());
        let s3_transfer = aws_sdk_s3::Client::from_conf(
            s3_base()
                .timeout_config(
                    TimeoutConfig::builder()
                        .connect_timeout(Duration::from_secs(10))
                        .build(),
                )
                .build(),
        );
        let s3_bulk = aws_sdk_s3::Client::from_conf(
            s3_base()
                .retry_config(RetryConfig::adaptive().with_max_attempts(5))
                .build(),
        );
        let mut cloudwatch = aws_sdk_cloudwatch::config::Builder::from(config);
        let global = Region::from_static(GLOBAL_REGION);
        let mut cost = aws_sdk_costexplorer::config::Builder::from(config).region(global.clone());
        let mut pricing = aws_sdk_pricing::config::Builder::from(config).region(global);
        if let Some(r) = &expired_retry {
            cloudwatch = cloudwatch.retry_classifier(r.clone());
            cost = cost.retry_classifier(r.clone());
            pricing = pricing.retry_classifier(r.clone());
        }
        let cloudwatch = aws_sdk_cloudwatch::Client::from_conf(cloudwatch.build());
        let cost = aws_sdk_costexplorer::Client::from_conf(cost.build());
        let pricing = aws_sdk_pricing::Client::from_conf(pricing.build());
        Self {
            s3,
            s3_transfer,
            s3_bulk,
            cloudwatch,
            cost,
            pricing,
            full_checksums: endpoint_url.is_none(),
        }
    }
}

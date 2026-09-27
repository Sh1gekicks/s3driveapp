//! 接続ごとの SDK クライアント（01 §6.1）。

pub mod error_map;

use std::time::Duration;

use aws_config::{BehaviorVersion, Region, SdkConfig};
use aws_credential_types::provider::SharedCredentialsProvider;
use aws_sdk_s3::config::{RequestChecksumCalculation, ResponseChecksumValidation};
use aws_smithy_types::retry::RetryConfig;
use aws_smithy_types::timeout::TimeoutConfig;

/// Cost Explorer と Price List のエンドポイントのリージョン。
pub const GLOBAL_REGION: &str = "us-east-1";

/// 共通の SDK 設定を作る。認証情報は明示したプロバイダだけを使う。
pub async fn sdk_config(
    region: &str,
    credentials: SharedCredentialsProvider,
    endpoint_url: Option<&str>,
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
    pub fn new(config: &SdkConfig, endpoint_url: Option<&str>) -> Self {
        let s3_base = || {
            let mut b = aws_sdk_s3::config::Builder::from(config);
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
        let cloudwatch = aws_sdk_cloudwatch::Client::new(config);
        let global = Region::from_static(GLOBAL_REGION);
        let cost = aws_sdk_costexplorer::Client::from_conf(
            aws_sdk_costexplorer::config::Builder::from(config)
                .region(global.clone())
                .build(),
        );
        let pricing = aws_sdk_pricing::Client::from_conf(
            aws_sdk_pricing::config::Builder::from(config)
                .region(global)
                .build(),
        );
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

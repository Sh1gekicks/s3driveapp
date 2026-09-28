//! AWS 認証情報（04 §2、07 §3）。
//!
//! シークレットはキーチェーンにのみ保存し（`aws:{認証情報 ID}` に JSON）、IPC・ログには出さない。

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use aws_credential_types::Credentials;
use aws_credential_types::provider::{self, ProvideCredentials, error::CredentialsError};
use aws_smithy_runtime_api::client::interceptors::context::InterceptorContext;
use aws_smithy_runtime_api::client::retries::classifiers::{ClassifyRetry, RetryAction};
use serde::{Deserialize, Serialize};

use crate::aws::error_map::{self, Ctx};
use crate::error::{CoreError, CoreResult, ErrorCode};
use crate::store::SecretStore;
use crate::store::secrets::aws_account;

/// キーチェーンに保存する値。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessKey {
    pub access_key_id: String,
    pub secret_access_key: String,
}

impl std::fmt::Debug for AccessKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccessKey")
            .field(
                "access_key_id",
                &crate::util::key::mask_access_key_id(&self.access_key_id),
            )
            .field("secret_access_key", &"********")
            .finish()
    }
}

impl AccessKey {
    pub fn new(access_key_id: &str, secret_access_key: &str) -> Self {
        Self {
            access_key_id: access_key_id.trim().to_string(),
            secret_access_key: secret_access_key.trim().to_string(),
        }
    }

    /// 入力値の形式を確認する（03 §4.2）。
    pub fn validate(&self) -> CoreResult<()> {
        let id_ok = self.access_key_id.len() == 20
            && self.access_key_id.starts_with("AKIA")
            && self
                .access_key_id
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
        if !id_ok {
            return Err(CoreError::with_message(
                ErrorCode::CredentialsInvalid,
                "アクセスキー ID の形式が正しくありません",
            )
            .detail("field=accessKeyId"));
        }
        if self.secret_access_key.len() != 40 {
            return Err(CoreError::with_message(
                ErrorCode::CredentialsInvalid,
                "シークレットアクセスキーの形式が正しくありません",
            )
            .detail("field=secretAccessKey"));
        }
        Ok(())
    }

    pub fn to_credentials(&self) -> Credentials {
        Credentials::new(
            &self.access_key_id,
            &self.secret_access_key,
            None,
            None,
            "s3drive-static",
        )
    }
}

pub fn load_access_key(secrets: &dyn SecretStore, credential_id: &str) -> CoreResult<AccessKey> {
    let raw = secrets.get(&aws_account(credential_id))?.ok_or_else(|| {
        CoreError::with_message(
            ErrorCode::CredentialsInvalid,
            "認証情報がキーチェーンにありません。認証情報を更新してください",
        )
    })?;
    serde_json::from_str(&raw).map_err(|_| CoreError::new(ErrorCode::CredentialsInvalid))
}

pub fn save_access_key(
    secrets: &dyn SecretStore,
    credential_id: &str,
    key: &AccessKey,
) -> CoreResult<()> {
    secrets.set(&aws_account(credential_id), &serde_json::to_string(key)?)
}

/// AssumeRole のセッション名（`s3drive-{メールアドレス}`。使えない文字は `-`、64 文字まで。04 §2.3）。
pub fn role_session_name(email: &str) -> String {
    let raw = format!("s3drive-{email}");
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "+=,.@-_".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect();
    cleaned.chars().take(64).collect()
}

/// ロール ARN の形式（`arn:aws:iam::<12 桁>:role/<名前>`）。
pub fn is_valid_role_arn(arn: &str) -> bool {
    let Some(rest) = arn.strip_prefix("arn:") else {
        return false;
    };
    let mut parts = rest.splitn(5, ':');
    let partition = parts.next().unwrap_or("");
    let service = parts.next().unwrap_or("");
    let region = parts.next().unwrap_or("");
    let account = parts.next().unwrap_or("");
    let resource = parts.next().unwrap_or("");
    partition.starts_with("aws")
        && service == "iam"
        && region.is_empty()
        && account.len() == 12
        && account.chars().all(|c| c.is_ascii_digit())
        && resource
            .strip_prefix("role/")
            .is_some_and(|n| !n.is_empty())
}

/// AssumeRole の設定。
#[derive(Debug, Clone)]
pub struct AssumeRoleParams {
    pub role_arn: String,
    pub session_name: String,
    pub external_id: Option<String>,
    pub source_identity: Option<String>,
    pub duration: Duration,
}

/// 一時認証情報の期限がこの時間より近ければ、使わずに取り直す。
const REFRESH_BEFORE_EXPIRY: Duration = Duration::from_secs(5 * 60);
/// 取り直してからこの時間内に `ExpiredToken` が返った場合は、取り直しても解決しないため再試行しない。
const JUST_REFRESHED: Duration = Duration::from_secs(60);

/// 取得済みの一時認証情報（取得した時刻とともに保持する）。
#[derive(Default)]
struct RoleCache {
    current: Mutex<Option<(Credentials, Instant)>>,
    /// 同時に要求が来ても AssumeRole を 1 回にまとめる。
    refresh: tokio::sync::Mutex<()>,
}

/// AssumeRole で一時認証情報を得るプロバイダ（04 §2.3）。
///
/// - SDK の `AssumeRoleProvider` は `SourceIdentity` を指定できないため、自前で実装する。
/// - 一時認証情報はこのプロバイダがキャッシュし、期限の 5 分前に取り直す。`ExpiredToken` が返ったときに
///   キャッシュを捨てて取り直せるよう、SDK の ID キャッシュは使わない（[`ExpiredTokenRetry`]。01 §7.1）。
#[derive(Clone)]
pub struct AssumeRoleCredentials {
    sts: aws_sdk_sts::Client,
    params: Arc<AssumeRoleParams>,
    cache: Arc<RoleCache>,
}

impl std::fmt::Debug for AssumeRoleCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssumeRoleCredentials")
            .field("role_arn", &self.params.role_arn)
            .finish()
    }
}

impl AssumeRoleCredentials {
    pub fn new(sts: aws_sdk_sts::Client, params: AssumeRoleParams) -> Self {
        Self {
            sts,
            params: Arc::new(params),
            cache: Arc::new(RoleCache::default()),
        }
    }

    /// キャッシュした一時認証情報を返す。なければ（期限が近ければ）AssumeRole で取り直す。
    pub async fn credentials(&self) -> CoreResult<Credentials> {
        if let Some(c) = self.cached() {
            return Ok(c);
        }
        let _refreshing = self.cache.refresh.lock().await;
        if let Some(c) = self.cached() {
            return Ok(c);
        }
        let c = self.assume().await?;
        *self.cache.current.lock().unwrap() = Some((c.clone(), Instant::now()));
        Ok(c)
    }

    /// テスト用。キャッシュした一時認証情報を、`age` だけ前に取得したことにする。
    #[cfg(test)]
    fn age_cache(&self, age: Duration) {
        if let Some((_, fetched)) = self.cache.current.lock().unwrap().as_mut() {
            *fetched -= age;
        }
    }

    fn cached(&self) -> Option<Credentials> {
        let current = self.cache.current.lock().unwrap();
        let (c, _) = current.as_ref()?;
        c.expiry()
            .is_none_or(|e| e > SystemTime::now() + REFRESH_BEFORE_EXPIRY)
            .then(|| c.clone())
    }

    /// `ExpiredToken` を受け取ったときに呼ぶ。キャッシュを捨てて次の試行で取り直す場合は真を返す。
    ///
    /// 取り直した直後の認証情報でも拒否された場合（時刻のずれなど）は、何度取り直しても解決しないため偽を返す。
    /// これにより、1 つの要求の再試行は 1 回に限られる（01 §7.1）。
    pub fn on_expired_token(&self) -> bool {
        let mut current = self.cache.current.lock().unwrap();
        match current.as_ref() {
            Some((_, fetched)) if fetched.elapsed() < JUST_REFRESHED => false,
            _ => {
                *current = None;
                true
            }
        }
    }

    /// 一時認証情報を取得する（キャッシュしない）。接続の確認では、このエラーを `ROLE_ASSUME_DENIED` として表示する。
    pub async fn assume(&self) -> CoreResult<Credentials> {
        let p = &self.params;
        let out = self
            .sts
            .assume_role()
            .role_arn(&p.role_arn)
            .role_session_name(&p.session_name)
            .set_external_id(p.external_id.clone())
            .set_source_identity(p.source_identity.clone())
            .duration_seconds(p.duration.as_secs() as i32)
            .send()
            .await
            .map_err(|e| error_map::classify(&e, Ctx::AssumeRole))?;
        let c = out
            .credentials()
            .ok_or_else(|| CoreError::new(ErrorCode::RoleAssumeDenied).detail("no credentials"))?;
        let expiry = SystemTime::try_from(*c.expiration()).ok();
        Ok(Credentials::new(
            c.access_key_id(),
            c.secret_access_key(),
            Some(c.session_token().to_string()),
            expiry,
            "s3drive-assume-role",
        ))
    }
}

impl ProvideCredentials for AssumeRoleCredentials {
    fn provide_credentials<'a>(&'a self) -> provider::future::ProvideCredentials<'a>
    where
        Self: 'a,
    {
        provider::future::ProvideCredentials::new(async move {
            self.credentials()
                .await
                .map_err(CredentialsError::provider_error)
        })
    }
}

/// `ExpiredToken` が返ったら一時認証情報を取り直し、1 回だけ再試行させるリトライ分類器（01 §7.1）。
///
/// AssumeRole を使う接続の各クライアントに登録する。静的なアクセスキーは期限切れにならないため登録しない。
#[derive(Clone, Debug)]
pub struct ExpiredTokenRetry(AssumeRoleCredentials);

impl ExpiredTokenRetry {
    pub fn new(credentials: AssumeRoleCredentials) -> Self {
        Self(credentials)
    }
}

impl ClassifyRetry for ExpiredTokenRetry {
    fn classify_retry(&self, ctx: &InterceptorContext) -> RetryAction {
        if !matches!(ctx.output_or_error(), Some(Err(_))) {
            return RetryAction::NoActionIndicated;
        }
        let Some(response) = ctx.response() else {
            return RetryAction::NoActionIndicated;
        };
        let expired = error_map::is_expired_token_response(
            response.status().as_u16(),
            response.headers().get("x-amzn-errortype"),
            response.body().bytes(),
        );
        if expired && self.0.on_expired_token() {
            log::info!("一時認証情報の期限が切れたため、取り直して再試行します");
            RetryAction::transient_error()
        } else {
            RetryAction::NoActionIndicated
        }
    }

    fn name(&self) -> &'static str {
        "ExpiredTokenRetry"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // テスト用の架空の値。シークレットスキャンに実在のキーと誤検知されないよう、アクセスキー ID は分けて書く
    const KEY_ID: &str = concat!("AKIA", "TESTFAKEKEY00000");
    const SECRET: &str = "test-secret-access-key-for-s3drive-00000";

    #[test]
    fn validates_access_key_format() {
        assert!(AccessKey::new(KEY_ID, &"a".repeat(40)).validate().is_ok());
        assert!(
            AccessKey::new("AKIA123", &"a".repeat(40))
                .validate()
                .is_err()
        );
        assert!(
            AccessKey::new(&KEY_ID.to_lowercase(), &"a".repeat(40))
                .validate()
                .is_err()
        );
        assert!(AccessKey::new(KEY_ID, "short").validate().is_err());
    }

    #[test]
    fn debug_output_masks_secrets() {
        let key = AccessKey::new(KEY_ID, SECRET);
        let dbg = format!("{key:?}");
        assert!(!dbg.contains("test-secret"));
        assert!(!dbg.contains("FAKEKEY"));
    }

    #[test]
    fn builds_role_session_names() {
        assert_eq!(
            role_session_name("yuki.tanaka@gmail.com"),
            "s3drive-yuki.tanaka@gmail.com"
        );
        assert_eq!(
            role_session_name("田中@example.com"),
            "s3drive---@example.com"
        );
        assert_eq!(
            role_session_name(&format!("{}@x.com", "a".repeat(100))).len(),
            64
        );
    }

    #[test]
    fn validates_role_arns() {
        assert!(is_valid_role_arn(
            "arn:aws:iam::123456789012:role/S3DriveAccess"
        ));
        assert!(is_valid_role_arn(
            "arn:aws:iam::123456789012:role/path/Name"
        ));
        assert!(!is_valid_role_arn("arn:aws:iam::12345:role/x"));
        assert!(!is_valid_role_arn("arn:aws:iam::123456789012:user/x"));
        assert!(!is_valid_role_arn("arn:aws:s3:::bucket"));
    }

    mod expired_token {
        use std::sync::atomic::{AtomicUsize, Ordering};

        use aws_credential_types::provider::SharedCredentialsProvider;
        use aws_sdk_s3::operation::list_objects_v2::ListObjectsV2Output;
        use aws_sdk_sts::operation::assume_role::AssumeRoleOutput;
        use aws_smithy_mocks::{Rule, RuleMode, mock, mock_client};
        use aws_smithy_runtime_api::client::orchestrator::HttpResponse;
        use aws_smithy_runtime_api::http::StatusCode;
        use aws_smithy_types::body::SdkBody;
        use aws_smithy_types::retry::RetryConfig;

        use super::*;
        use crate::aws::error_map::{self, Ctx};

        static ISSUED: AtomicUsize = AtomicUsize::new(0);

        /// AssumeRole のたびに別のアクセスキーを返す STS。
        fn role() -> (AssumeRoleCredentials, Rule) {
            let rule = mock!(aws_sdk_sts::Client::assume_role).then_output(|| {
                let n = ISSUED.fetch_add(1, Ordering::SeqCst);
                AssumeRoleOutput::builder()
                    .credentials(
                        aws_sdk_sts::types::Credentials::builder()
                            .access_key_id(format!("ASIATEMP{n}"))
                            .secret_access_key("temp-secret")
                            .session_token("temp-token")
                            .expiration(aws_smithy_types::DateTime::from(
                                SystemTime::now() + Duration::from_secs(3600),
                            ))
                            .build()
                            .unwrap(),
                    )
                    .build()
            });
            let sts = mock_client!(aws_sdk_sts, RuleMode::MatchAny, [&rule]);
            let creds = AssumeRoleCredentials::new(
                sts,
                AssumeRoleParams {
                    role_arn: "arn:aws:iam::123456789012:role/S3DriveAccess".into(),
                    session_name: "s3drive-test".into(),
                    external_id: None,
                    source_identity: None,
                    duration: Duration::from_secs(3600),
                },
            );
            (creds, rule)
        }

        fn expired() -> HttpResponse {
            HttpResponse::new(
                StatusCode::try_from(400).unwrap(),
                SdkBody::from(
                    "<Error><Code>ExpiredToken</Code><Message>The provided token has expired.</Message></Error>",
                ),
            )
        }

        /// 接続のクライアントと同じ構成（ID キャッシュなし、分類器あり）の S3 クライアント。
        fn s3(role: &AssumeRoleCredentials, rule: &Rule) -> aws_sdk_s3::Client {
            let role = role.clone();
            mock_client!(aws_sdk_s3, RuleMode::Sequential, [rule], move |b| b
                .credentials_provider(SharedCredentialsProvider::new(role.clone()))
                .identity_cache(aws_config::identity::IdentityCache::no_cache())
                .retry_config(
                    RetryConfig::standard()
                        .with_max_attempts(3)
                        .with_initial_backoff(Duration::from_millis(1))
                )
                .retry_classifier(ExpiredTokenRetry::new(role.clone())))
        }

        #[tokio::test]
        async fn refreshes_credentials_and_retries_once() {
            let (role, sts) = role();
            // 50 分前に取得した一時認証情報が、時刻のずれなどで期限切れと判定された
            role.credentials().await.unwrap();
            role.age_cache(Duration::from_secs(50 * 60));
            let list = mock!(aws_sdk_s3::Client::list_objects_v2)
                .sequence()
                .http_response(expired)
                .output(|| ListObjectsV2Output::builder().key_count(0).build())
                .build();
            let out = s3(&role, &list)
                .list_objects_v2()
                .bucket("b")
                .send()
                .await
                .unwrap();
            assert_eq!(out.key_count(), Some(0));
            // 最初の取得と、ExpiredToken を受けての取り直し
            assert_eq!(sts.num_calls(), 2);
            assert_eq!(list.num_calls(), 2);
        }

        #[tokio::test]
        async fn does_not_retry_again_with_fresh_credentials() {
            let (role, sts) = role();
            role.credentials().await.unwrap();
            role.age_cache(Duration::from_secs(50 * 60));
            let list = mock!(aws_sdk_s3::Client::list_objects_v2)
                .sequence()
                .http_response(expired)
                .repeatedly()
                .build();
            let err = s3(&role, &list)
                .list_objects_v2()
                .bucket("b")
                .send()
                .await
                .unwrap_err();
            // 取り直した直後の認証情報でも拒否されたら、それ以上は再試行しない
            assert_eq!(list.num_calls(), 2);
            assert_eq!(sts.num_calls(), 2);
            assert_eq!(
                error_map::classify(&err, Ctx::Op("s3:ListBucket")).code,
                ErrorCode::CredentialsExpired
            );
        }

        #[tokio::test]
        async fn caches_credentials_until_shortly_before_expiry() {
            let (role, sts) = role();
            let a = role.credentials().await.unwrap();
            let b = role.credentials().await.unwrap();
            assert_eq!(a.access_key_id(), b.access_key_id());
            assert_eq!(sts.num_calls(), 1);
        }
    }

    #[test]
    fn stores_access_keys_as_json() {
        let store = crate::store::MemorySecretStore::new();
        let key = AccessKey::new(KEY_ID, SECRET);
        save_access_key(&store, "c1", &key).unwrap();
        let loaded = load_access_key(&store, "c1").unwrap();
        assert_eq!(loaded.access_key_id, key.access_key_id);
        assert!(load_access_key(&store, "missing").is_err());
    }
}

//! AWS 認証情報（04 §2、07 §3）。
//!
//! シークレットはキーチェーンにのみ保存し（`aws:{認証情報 ID}` に JSON）、IPC・ログには出さない。

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use aws_credential_types::Credentials;
use aws_credential_types::provider::{self, ProvideCredentials, error::CredentialsError};
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

/// AssumeRole で一時認証情報を得るプロバイダ。期限前の再取得は SDK の ID キャッシュに任せる（04 §2.3）。
///
/// SDK の `AssumeRoleProvider` は `SourceIdentity` を指定できないため、自前で実装する。
#[derive(Clone)]
pub struct AssumeRoleCredentials {
    sts: aws_sdk_sts::Client,
    params: Arc<AssumeRoleParams>,
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
        }
    }

    /// 一時認証情報を取得する。接続の確認では、このエラーを `ROLE_ASSUME_DENIED` として表示する。
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
            self.assume()
                .await
                .map_err(CredentialsError::provider_error)
        })
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

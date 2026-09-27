//! バケット接続と認証情報の管理（04 §2）。

use std::sync::Arc;
use std::time::{Duration, Instant};

use aws_credential_types::provider::SharedCredentialsProvider;

use crate::Core;
use crate::aws::error_map::{self, Ctx};
use crate::aws::{self, Clients};
use crate::credentials::{
    self, AccessKey, AssumeRoleCredentials, AssumeRoleParams, load_access_key, save_access_key,
};
use crate::error::{CoreError, CoreResult, ErrorCode};
use crate::model::{
    Account, BucketInfo, Connection, ConnectionId, ConnectionInput, ConnectionPatch,
    ConnectionRecord, ConnectionTestResult, CredentialInput, CredentialRecord, CredentialSummary,
    Location, UserSession, Versioning,
};
use crate::store::secrets::aws_account;
use crate::util::{key, region, time};

const BUCKET_INFO_TTL: Duration = Duration::from_secs(10 * 60);
const ROLE_SESSION_DURATION: Duration = Duration::from_secs(60 * 60);

/// 接続ごとの実行時の状態（SDK クライアントとバケット情報のキャッシュ）。
pub struct ConnCtx {
    pub id: ConnectionId,
    pub bucket: String,
    pub region: String,
    pub record: ConnectionRecord,
    pub clients: Clients,
    info: tokio::sync::Mutex<Option<(BucketInfo, Instant)>>,
}

impl std::fmt::Debug for ConnCtx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnCtx")
            .field("id", &self.id)
            .field("bucket", &self.bucket)
            .field("region", &self.region)
            .finish()
    }
}

impl ConnCtx {
    /// バケットの情報（10 分キャッシュ）。
    pub async fn bucket_info(&self, force: bool) -> CoreResult<BucketInfo> {
        let mut guard = self.info.lock().await;
        if !force
            && let Some((info, at)) = guard.as_ref()
            && at.elapsed() < BUCKET_INFO_TTL
        {
            return Ok(info.clone());
        }
        let versioning = fetch_versioning(&self.clients.s3, &self.bucket).await;
        let encryption = fetch_encryption(&self.clients.s3, &self.bucket).await;
        let info = BucketInfo {
            bucket: self.bucket.clone(),
            region: self.region.clone(),
            versioning,
            encryption,
        };
        *guard = Some((info.clone(), Instant::now()));
        Ok(info)
    }

    pub async fn versioning(&self) -> Versioning {
        self.bucket_info(false)
            .await
            .map(|i| i.versioning)
            .unwrap_or(Versioning::Unknown)
    }
}

async fn fetch_versioning(s3: &aws_sdk_s3::Client, bucket: &str) -> Versioning {
    match s3.get_bucket_versioning().bucket(bucket).send().await {
        Ok(out) => match out.status() {
            Some(aws_sdk_s3::types::BucketVersioningStatus::Enabled) => Versioning::Enabled,
            Some(aws_sdk_s3::types::BucketVersioningStatus::Suspended) => Versioning::Suspended,
            _ => Versioning::Disabled,
        },
        Err(e) => {
            // 403 などは「不明」として続行する（04 §2.2）
            log::warn!(
                "GetBucketVersioning に失敗: {}",
                error_map::classify(&e, Ctx::Bucket)
            );
            Versioning::Unknown
        }
    }
}

async fn fetch_encryption(s3: &aws_sdk_s3::Client, bucket: &str) -> String {
    match s3.get_bucket_encryption().bucket(bucket).send().await {
        Ok(out) => out
            .server_side_encryption_configuration()
            .and_then(|c| c.rules().first())
            .and_then(|r| r.apply_server_side_encryption_by_default())
            .map(|d| encryption_label(d.sse_algorithm().as_str(), d.kms_master_key_id()))
            .unwrap_or_else(|| "なし".to_string()),
        Err(_) => "不明".to_string(),
    }
}

/// 暗号化方式の表示（03 §5.6）。
pub fn encryption_label(algorithm: &str, kms_key_id: Option<&str>) -> String {
    match algorithm {
        "AES256" => "SSE-S3 (AES-256)".to_string(),
        "aws:kms" => match kms_key_id {
            Some(k) => format!("SSE-KMS（{k}）"),
            None => "SSE-KMS".to_string(),
        },
        "aws:kms:dsse" => "DSSE-KMS".to_string(),
        "" => "なし".to_string(),
        other => other.to_string(),
    }
}

pub struct ConnectionService<'a> {
    core: &'a Core,
}

impl Core {
    pub fn connections(&self) -> ConnectionService<'_> {
        ConnectionService { core: self }
    }

    fn endpoint_for(&self, record: &ConnectionRecord) -> Option<String> {
        self.0.endpoint_override.clone().or_else(|| {
            self.0
                .allow_connection_endpoints
                .then(|| record.endpoint_url.clone())
                .flatten()
        })
    }

    /// サインイン中のアカウントの接続を解決し、SDK クライアントを返す（07 §5.1 の入力検証を兼ねる）。
    pub(crate) async fn ctx(&self, connection_id: &str) -> CoreResult<Arc<ConnCtx>> {
        let account = self.account()?;
        if let Some(ctx) = self.0.clients.lock().unwrap().get(connection_id) {
            return Ok(ctx.clone());
        }
        let record = self
            .0
            .settings
            .read(|f| {
                f.accounts.get(&account.sub).and_then(|a| {
                    a.connections
                        .iter()
                        .find(|c| c.id == connection_id)
                        .cloned()
                })
            })
            .ok_or_else(|| CoreError::with_message(ErrorCode::NotFound, "接続が見つかりません"))?;
        let ctx = Arc::new(self.build_ctx(&account, record).await?);
        self.0
            .clients
            .lock()
            .unwrap()
            .insert(connection_id.to_string(), ctx.clone());
        Ok(ctx)
    }

    async fn build_ctx(
        &self,
        account: &UserSession,
        record: ConnectionRecord,
    ) -> CoreResult<ConnCtx> {
        let key = load_access_key(self.0.secrets.as_ref(), &record.credential_id)?;
        let endpoint = self.endpoint_for(&record);
        let provider = credentials_provider(account, &record, &key, endpoint.as_deref()).await;
        let config = aws::sdk_config(&record.region, provider, endpoint.as_deref()).await;
        Ok(ConnCtx {
            id: record.id.clone(),
            bucket: record.bucket.clone(),
            region: record.region.clone(),
            clients: Clients::new(&config, endpoint.as_deref()),
            record,
            info: tokio::sync::Mutex::new(None),
        })
    }

    pub(crate) fn invalidate_clients(&self, connection_id: &str) {
        self.0.clients.lock().unwrap().remove(connection_id);
    }
}

async fn credentials_provider(
    account: &UserSession,
    record: &ConnectionRecord,
    key: &AccessKey,
    endpoint: Option<&str>,
) -> SharedCredentialsProvider {
    let static_provider = SharedCredentialsProvider::new(key.to_credentials());
    let Some(role_arn) = &record.role_arn else {
        return static_provider;
    };
    let base = aws::sdk_config(&record.region, static_provider, endpoint).await;
    let sts = aws_sdk_sts::Client::new(&base);
    SharedCredentialsProvider::new(AssumeRoleCredentials::new(
        sts,
        AssumeRoleParams {
            role_arn: role_arn.clone(),
            session_name: credentials::role_session_name(&account.email),
            external_id: record.external_id.clone(),
            source_identity: record.use_source_identity.then(|| account.email.clone()),
            duration: ROLE_SESSION_DURATION,
        },
    ))
}

fn to_connection(record: &ConnectionRecord, credential: Option<&CredentialRecord>) -> Connection {
    Connection {
        id: record.id.clone(),
        bucket: record.bucket.clone(),
        region: record.region.clone(),
        region_label: region::label(&record.region),
        region_short: region::short(&record.region),
        credential_id: record.credential_id.clone(),
        access_key_id_masked: credential
            .map(|c| c.access_key_id_masked.clone())
            .unwrap_or_default(),
        role_arn: record.role_arn.clone(),
        external_id: record.external_id.clone(),
        use_source_identity: record.use_source_identity,
        cost_tag: record.cost_tag.clone(),
        default_storage_class: record.default_storage_class,
    }
}

fn non_empty(s: &Option<String>) -> Option<String> {
    s.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

impl ConnectionService<'_> {
    fn with_account<R>(&self, f: impl FnOnce(&Account) -> R) -> CoreResult<R> {
        let sub = self.core.account()?.sub;
        Ok(self.core.0.settings.read(|file| {
            let empty = Account::default();
            f(file.accounts.get(&sub).unwrap_or(&empty))
        }))
    }

    fn update_account<R>(&self, f: impl FnOnce(&mut Account) -> CoreResult<R>) -> CoreResult<R> {
        let session = self.core.account()?;
        self.core.0.settings.update(|file| {
            let account = file.accounts.entry(session.sub.clone()).or_default();
            if account.profile.email.is_empty() {
                account.profile.email = session.email.clone();
                account.profile.name = session.name.clone();
            }
            f(account)
        })
    }

    pub fn list(&self) -> CoreResult<Vec<Connection>> {
        self.with_account(|a| {
            a.connections
                .iter()
                .map(|c| to_connection(c, a.credentials.iter().find(|k| k.id == c.credential_id)))
                .collect()
        })
    }

    pub fn get(&self, id: &str) -> CoreResult<Connection> {
        self.list()?
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| CoreError::with_message(ErrorCode::NotFound, "接続が見つかりません"))
    }

    fn resolve_key(&self, credential: &CredentialInput) -> CoreResult<AccessKey> {
        match credential {
            CredentialInput::New {
                access_key_id,
                secret_access_key,
            } => {
                let key = AccessKey::new(access_key_id, secret_access_key);
                key.validate()?;
                Ok(key)
            }
            CredentialInput::Existing { credential_id } => {
                let owned =
                    self.with_account(|a| a.credentials.iter().any(|c| &c.id == credential_id))?;
                if !owned {
                    return Err(CoreError::with_message(
                        ErrorCode::NotFound,
                        "認証情報が見つかりません",
                    ));
                }
                load_access_key(self.core.0.secrets.as_ref(), credential_id)
            }
        }
    }

    fn validate_input(input: &ConnectionInput) -> CoreResult<()> {
        if !key::is_valid_bucket_name(input.bucket.trim()) {
            return Err(CoreError::with_message(
                ErrorCode::BucketNotFound,
                "バケット名の形式が正しくありません",
            ));
        }
        if input.region.trim().is_empty() {
            return Err(CoreError::with_message(
                ErrorCode::InvalidName,
                "リージョンを選択してください",
            ));
        }
        if let Some(arn) = non_empty(&input.role_arn)
            && !credentials::is_valid_role_arn(&arn)
        {
            return Err(CoreError::with_message(
                ErrorCode::RoleAssumeDenied,
                "IAM ロール ARN の形式が正しくありません",
            ));
        }
        Ok(())
    }

    /// 接続の確認（04 §2.2）。GetCallerIdentity → AssumeRole（任意）→ HeadBucket → GetBucketVersioning。
    pub async fn test(&self, input: &ConnectionInput) -> CoreResult<ConnectionTestResult> {
        Ok(self.test_inner(input).await?.0)
    }

    async fn test_inner(
        &self,
        input: &ConnectionInput,
    ) -> CoreResult<(ConnectionTestResult, AccessKey)> {
        Self::validate_input(input)?;
        let session = self.core.account()?;
        let key = self.resolve_key(&input.credential)?;
        let bucket = input.bucket.trim().to_string();
        let mut region = input.region.trim().to_string();
        let role_arn = non_empty(&input.role_arn);
        let external_id = non_empty(&input.external_id);
        let endpoint = self.core.0.endpoint_override.clone();

        let static_provider = SharedCredentialsProvider::new(key.to_credentials());
        let base = aws::sdk_config(&region, static_provider.clone(), endpoint.as_deref()).await;
        let sts = aws_sdk_sts::Client::new(&base);
        let caller = sts
            .get_caller_identity()
            .send()
            .await
            .map_err(|e| error_map::classify(&e, Ctx::Caller))?;
        let caller_arn = caller.arn().unwrap_or_default().to_string();
        if caller_arn.ends_with(":root") {
            return Err(CoreError::with_message(
                ErrorCode::CredentialsInvalid,
                "ルートユーザーのアクセスキーは使用できません。IAM ユーザーを作成してください",
            ));
        }

        let provider = match &role_arn {
            None => static_provider,
            Some(arn) => {
                let assumed = AssumeRoleCredentials::new(
                    sts.clone(),
                    AssumeRoleParams {
                        role_arn: arn.clone(),
                        session_name: credentials::role_session_name(&session.email),
                        external_id: external_id.clone(),
                        source_identity: None,
                        duration: Duration::from_secs(15 * 60),
                    },
                )
                .assume()
                .await?;
                SharedCredentialsProvider::new(assumed)
            }
        };

        let mut region_corrected = false;
        let mut s3 = Clients::new(
            &aws::sdk_config(&region, provider.clone(), endpoint.as_deref()).await,
            endpoint.as_deref(),
        )
        .s3;
        let head = match s3.head_bucket().bucket(&bucket).send().await {
            Ok(out) => Ok(out.bucket_region().map(str::to_string)),
            Err(e) => match error_map::bucket_region_hint(&e) {
                Some(actual) if actual != region => Ok(Some(actual)),
                _ => Err(error_map::classify(&e, Ctx::Bucket)),
            },
        }?;
        if let Some(actual) = head
            && !actual.is_empty()
            && actual != region
        {
            // リージョン違い: 正しいリージョンで確認し直す（03 §4.3）
            region = actual;
            region_corrected = true;
            s3 = Clients::new(
                &aws::sdk_config(&region, provider.clone(), endpoint.as_deref()).await,
                endpoint.as_deref(),
            )
            .s3;
            s3.head_bucket()
                .bucket(&bucket)
                .send()
                .await
                .map_err(|e| error_map::classify(&e, Ctx::Bucket))?;
        }
        let versioning = fetch_versioning(&s3, &bucket).await;
        Ok((
            ConnectionTestResult {
                account_id: caller.account().unwrap_or_default().to_string(),
                caller_arn,
                region,
                region_corrected,
                versioning,
            },
            key,
        ))
    }

    /// 確認してから保存する。
    pub async fn create(&self, input: ConnectionInput) -> CoreResult<Connection> {
        let (result, key) = self.test_inner(&input).await?;
        let connection_id = uuid::Uuid::new_v4().to_string();
        let (credential_id, new_credential) = match &input.credential {
            CredentialInput::New { .. } => {
                let id = uuid::Uuid::new_v4().to_string();
                save_access_key(self.core.0.secrets.as_ref(), &id, &key)?;
                (id, true)
            }
            CredentialInput::Existing { credential_id } => (credential_id.clone(), false),
        };
        let record = ConnectionRecord {
            id: connection_id.clone(),
            bucket: input.bucket.trim().to_string(),
            region: result.region.clone(),
            credential_id: credential_id.clone(),
            role_arn: non_empty(&input.role_arn),
            external_id: non_empty(&input.external_id),
            use_source_identity: false,
            cost_tag: None,
            default_storage_class: None,
            endpoint_url: None,
        };
        let saved = self.update_account(|account| {
            if new_credential {
                account.credentials.push(CredentialRecord {
                    id: credential_id.clone(),
                    access_key_id_masked: key::mask_access_key_id(&key.access_key_id),
                    created_at: time::now_rfc3339(),
                });
            }
            account.connections.push(record.clone());
            Ok(to_connection(
                &record,
                account.credentials.iter().find(|c| c.id == credential_id),
            ))
        });
        if saved.is_err() && new_credential {
            let _ = self.core.0.secrets.delete(&aws_account(&credential_id));
        }
        saved
    }

    /// 接続の編集。確認をやり直してから保存する（04 §2.4）。
    pub async fn update(&self, id: &str, input: ConnectionInput) -> CoreResult<Connection> {
        let (result, key) = self.test_inner(&input).await?;
        let (credential_id, new_credential) = match &input.credential {
            CredentialInput::New { .. } => {
                let cid = uuid::Uuid::new_v4().to_string();
                save_access_key(self.core.0.secrets.as_ref(), &cid, &key)?;
                (cid, true)
            }
            CredentialInput::Existing { credential_id } => (credential_id.clone(), false),
        };
        let (connection, orphan) = self.update_account(|account| {
            if new_credential {
                account.credentials.push(CredentialRecord {
                    id: credential_id.clone(),
                    access_key_id_masked: key::mask_access_key_id(&key.access_key_id),
                    created_at: time::now_rfc3339(),
                });
            }
            let record = account
                .connections
                .iter_mut()
                .find(|c| c.id == id)
                .ok_or_else(|| {
                    CoreError::with_message(ErrorCode::NotFound, "接続が見つかりません")
                })?;
            let previous_credential =
                std::mem::replace(&mut record.credential_id, credential_id.clone());
            record.bucket = input.bucket.trim().to_string();
            record.region = result.region.clone();
            record.role_arn = non_empty(&input.role_arn);
            record.external_id = non_empty(&input.external_id);
            let record = record.clone();
            let orphan = remove_unused_credential(account, &previous_credential);
            Ok((
                to_connection(
                    &record,
                    account.credentials.iter().find(|c| c.id == credential_id),
                ),
                orphan,
            ))
        })?;
        if let Some(cid) = orphan {
            let _ = self.core.0.secrets.delete(&aws_account(&cid));
        }
        self.core.invalidate_clients(id);
        Ok(connection)
    }

    /// 接続ごとの設定（コスト配分タグ・既定のストレージクラスなど）を変更する。
    pub fn patch(&self, id: &str, patch: ConnectionPatch) -> CoreResult<Connection> {
        let connection = self.update_account(|account| {
            let record = account
                .connections
                .iter_mut()
                .find(|c| c.id == id)
                .ok_or_else(|| {
                    CoreError::with_message(ErrorCode::NotFound, "接続が見つかりません")
                })?;
            if let Some(tag) = patch.cost_tag {
                record.cost_tag = tag.filter(|t| !t.key.trim().is_empty());
            }
            if let Some(class) = patch.default_storage_class {
                record.default_storage_class = class;
            }
            if let Some(v) = patch.use_source_identity {
                record.use_source_identity = v;
            }
            let record = record.clone();
            Ok(to_connection(
                &record,
                account
                    .credentials
                    .iter()
                    .find(|c| c.id == record.credential_id),
            ))
        })?;
        self.core.invalidate_clients(id);
        Ok(connection)
    }

    /// 接続と関連データを削除する（06 §7）。
    pub async fn delete(&self, id: &str) -> CoreResult<()> {
        self.core.0.jobs.cancel_connection(id);
        self.core.0.transfers.cancel_connection(id);
        let orphan = self.update_account(|account| {
            let pos = account
                .connections
                .iter()
                .position(|c| c.id == id)
                .ok_or_else(|| {
                    CoreError::with_message(ErrorCode::NotFound, "接続が見つかりません")
                })?;
            let removed = account.connections.remove(pos);
            if account
                .last_location
                .as_ref()
                .is_some_and(|l| l.connection_id == id)
            {
                account.last_location = None;
            }
            Ok(remove_unused_credential(account, &removed.credential_id))
        })?;
        if let Some(cid) = orphan {
            self.core.0.secrets.delete(&aws_account(&cid))?;
        }
        self.core.invalidate_clients(id);
        let cid = id.to_string();
        self.core
            .0
            .db
            .run(move |c| {
                let tx = c.transaction()?;
                for table in [
                    "objects",
                    "prefixes",
                    "index_state",
                    "metrics_cache",
                    "restore_requests",
                    "transfers",
                ] {
                    tx.execute(
                        &format!("DELETE FROM {table} WHERE connection_id = ?1"),
                        [&cid],
                    )?;
                }
                tx.commit()?;
                Ok(())
            })
            .await
    }

    pub fn reorder(&self, ids: &[String]) -> CoreResult<()> {
        self.update_account(|account| {
            account
                .connections
                .sort_by_key(|c| ids.iter().position(|i| i == &c.id).unwrap_or(usize::MAX));
            Ok(())
        })
    }

    pub fn credential_list(&self) -> CoreResult<Vec<CredentialSummary>> {
        self.with_account(|a| {
            a.credentials
                .iter()
                .map(|c| CredentialSummary {
                    id: c.id.clone(),
                    access_key_id_masked: c.access_key_id_masked.clone(),
                    used_by: a
                        .connections
                        .iter()
                        .filter(|k| k.credential_id == c.id)
                        .map(|k| k.bucket.clone())
                        .collect(),
                })
                .collect()
        })
    }

    /// 認証情報の更新（DLG-06）。GetCallerIdentity で確認してからキーチェーンを上書きする。
    pub async fn credential_update(
        &self,
        credential_id: &str,
        access_key_id: &str,
        secret_access_key: &str,
    ) -> CoreResult<()> {
        let key = AccessKey::new(access_key_id, secret_access_key);
        key.validate()?;
        let (owned, region, users) = self.with_account(|a| {
            let users: Vec<String> = a
                .connections
                .iter()
                .filter(|c| c.credential_id == credential_id)
                .map(|c| c.id.clone())
                .collect();
            let region = a
                .connections
                .iter()
                .find(|c| c.credential_id == credential_id)
                .map(|c| c.region.clone())
                .unwrap_or_else(|| aws::GLOBAL_REGION.to_string());
            (
                a.credentials.iter().any(|c| c.id == credential_id),
                region,
                users,
            )
        })?;
        if !owned {
            return Err(CoreError::with_message(
                ErrorCode::NotFound,
                "認証情報が見つかりません",
            ));
        }
        let provider = SharedCredentialsProvider::new(key.to_credentials());
        let config =
            aws::sdk_config(&region, provider, self.core.0.endpoint_override.as_deref()).await;
        let caller = aws_sdk_sts::Client::new(&config)
            .get_caller_identity()
            .send()
            .await
            .map_err(|e| error_map::classify(&e, Ctx::Caller))?;
        if caller.arn().unwrap_or_default().ends_with(":root") {
            return Err(CoreError::with_message(
                ErrorCode::CredentialsInvalid,
                "ルートユーザーのアクセスキーは使用できません。IAM ユーザーを作成してください",
            ));
        }
        save_access_key(self.core.0.secrets.as_ref(), credential_id, &key)?;
        self.update_account(|account| {
            if let Some(c) = account
                .credentials
                .iter_mut()
                .find(|c| c.id == credential_id)
            {
                c.access_key_id_masked = key::mask_access_key_id(&key.access_key_id);
            }
            Ok(())
        })?;
        for id in users {
            self.core.invalidate_clients(&id);
        }
        Ok(())
    }

    pub async fn bucket_info(&self, connection_id: &str, force: bool) -> CoreResult<BucketInfo> {
        self.core.ctx(connection_id).await?.bucket_info(force).await
    }

    pub fn last_location(&self) -> CoreResult<Option<Location>> {
        self.with_account(|a| a.last_location.clone())
    }

    pub fn set_last_location(&self, location: Location) -> CoreResult<()> {
        key::validate_prefix(&location.prefix)?;
        self.update_account(|account| {
            if account
                .connections
                .iter()
                .any(|c| c.id == location.connection_id)
            {
                account.last_location = Some(location);
            }
            Ok(())
        })
    }
}

/// どの接続からも使われなくなった認証情報を記録から外し、その ID を返す。
fn remove_unused_credential(account: &mut Account, credential_id: &str) -> Option<String> {
    if account
        .connections
        .iter()
        .any(|c| c.credential_id == credential_id)
    {
        return None;
    }
    let before = account.credentials.len();
    account.credentials.retain(|c| c.id != credential_id);
    (account.credentials.len() != before).then(|| credential_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_encryption() {
        assert_eq!(encryption_label("AES256", None), "SSE-S3 (AES-256)");
        assert_eq!(encryption_label("aws:kms", Some("k1")), "SSE-KMS（k1）");
        assert_eq!(encryption_label("aws:kms", None), "SSE-KMS");
    }

    #[test]
    fn removes_only_unused_credentials() {
        let mut account = Account::default();
        account.credentials.push(CredentialRecord {
            id: "k1".into(),
            access_key_id_masked: "x".into(),
            created_at: "t".into(),
        });
        account.connections.push(ConnectionRecord {
            id: "c1".into(),
            bucket: "b".into(),
            region: "r".into(),
            credential_id: "k1".into(),
            role_arn: None,
            external_id: None,
            use_source_identity: false,
            cost_tag: None,
            default_storage_class: None,
            endpoint_url: None,
        });
        assert_eq!(remove_unused_credential(&mut account, "k1"), None);
        account.connections.clear();
        assert_eq!(
            remove_unused_credential(&mut account, "k1").as_deref(),
            Some("k1")
        );
        assert!(account.credentials.is_empty());
    }

    #[tokio::test]
    async fn requires_sign_in() {
        let (core, _dir) = Core::for_tests(None).unwrap();
        let err = core.connections().list().unwrap_err();
        assert_eq!(err.code, ErrorCode::AuthRequired);
    }

    #[tokio::test]
    async fn rejects_malformed_input_before_calling_aws() {
        let (core, _dir) = Core::for_tests(Some("http://127.0.0.1:9".into())).unwrap();
        core.set_session(Some(UserSession::new("sub", "a@example.com", "A")));
        let input = ConnectionInput {
            bucket: "Bad_Bucket".into(),
            region: "ap-northeast-1".into(),
            credential: CredentialInput::New {
                access_key_id: "AKIA4Z7XEXAMPLE7Q2LM".into(),
                secret_access_key: "s".repeat(40),
            },
            role_arn: None,
            external_id: None,
        };
        assert_eq!(
            core.connections().test(&input).await.unwrap_err().code,
            ErrorCode::BucketNotFound
        );
        let input = ConnectionInput {
            bucket: "good-bucket".into(),
            credential: CredentialInput::New {
                access_key_id: "bad".into(),
                secret_access_key: "s".repeat(40),
            },
            ..input
        };
        assert_eq!(
            core.connections().test(&input).await.unwrap_err().code,
            ErrorCode::CredentialsInvalid
        );
    }
}

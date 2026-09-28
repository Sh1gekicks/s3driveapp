use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::common::{ConnectionId, StorageClass, Versioning};

/// サインイン中の Google アカウント（05 §2）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UserSession {
    pub sub: String,
    pub email: String,
    pub name: String,
    /// アバターに表示する頭文字。
    pub initial: String,
}

impl UserSession {
    pub fn new(sub: impl Into<String>, email: impl Into<String>, name: impl Into<String>) -> Self {
        let email = email.into();
        let name = name.into();
        let initial = name
            .chars()
            .next()
            .or_else(|| email.chars().next())
            .map(|c| c.to_uppercase().collect::<String>())
            .unwrap_or_default();
        Self {
            sub: sub.into(),
            email,
            name,
            initial,
        }
    }
}

/// コスト配分タグ（04 §13.1）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CostTag {
    pub key: String,
    pub value: String,
}

/// 登録済みの接続（表示用）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Connection {
    pub id: ConnectionId,
    pub bucket: String,
    pub region: String,
    pub region_label: String,
    pub region_short: String,
    pub credential_id: String,
    pub access_key_id_masked: String,
    pub role_arn: Option<String>,
    pub external_id: Option<String>,
    pub use_source_identity: bool,
    pub cost_tag: Option<CostTag>,
    pub default_storage_class: Option<StorageClass>,
}

/// 認証情報の指定（新しいアクセスキー、または既存の認証情報）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum CredentialInput {
    #[serde(rename_all = "camelCase")]
    New {
        access_key_id: String,
        secret_access_key: String,
    },
    #[serde(rename_all = "camelCase")]
    Existing { credential_id: String },
}

/// 接続の入力値（SCR-01 ステップ 2、DLG-05）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConnectionInput {
    pub bucket: String,
    pub region: String,
    pub credential: CredentialInput,
    #[ts(optional)]
    #[serde(default)]
    pub role_arn: Option<String>,
    #[ts(optional)]
    #[serde(default)]
    pub external_id: Option<String>,
}

/// 接続の確認結果（04 §2.2）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConnectionTestResult {
    pub account_id: String,
    pub caller_arn: String,
    pub region: String,
    pub region_corrected: bool,
    pub versioning: Versioning,
}

/// 接続ごとの設定の変更（設定ウィンドウのコスト配分タグなど）。
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConnectionPatch {
    #[ts(optional)]
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::double_option"
    )]
    pub cost_tag: Option<Option<CostTag>>,
    #[ts(optional)]
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::double_option"
    )]
    pub default_storage_class: Option<Option<StorageClass>>,
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_source_identity: Option<bool>,
}

/// 既存の認証情報（DLG-05 の選択肢）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CredentialSummary {
    pub id: String,
    pub access_key_id_masked: String,
    /// この認証情報を使う接続のバケット名。
    pub used_by: Vec<String>,
}

/// バケットの情報（リージョン・バージョニング・暗号化）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BucketInfo {
    pub bucket: String,
    pub region: String,
    pub versioning: Versioning,
    pub encryption: String,
    /// HeadBucket の応答（301 と `x-amz-bucket-region`）から接続のリージョンを修正した（01 §6.1）。
    #[serde(default)]
    pub region_corrected: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_uses_first_character_of_name() {
        assert_eq!(
            UserSession::new("1", "yuki@example.com", "田中 優希").initial,
            "田"
        );
        assert_eq!(UserSession::new("1", "yuki@example.com", "").initial, "Y");
    }

    #[test]
    fn credential_input_is_tagged_by_kind() {
        let input: CredentialInput =
            serde_json::from_str(r#"{"kind":"new","accessKeyId":"AKIA","secretAccessKey":"x"}"#)
                .unwrap();
        assert!(matches!(input, CredentialInput::New { .. }));
        let input: CredentialInput =
            serde_json::from_str(r#"{"kind":"existing","credentialId":"c1"}"#).unwrap();
        assert!(
            matches!(input, CredentialInput::Existing { credential_id } if credential_id == "c1")
        );
    }

    #[test]
    fn connection_patch_distinguishes_clear_from_absent() {
        let patch: ConnectionPatch = serde_json::from_str(r#"{"costTag":null}"#).unwrap();
        assert_eq!(patch.cost_tag, Some(None));
        let patch: ConnectionPatch = serde_json::from_str("{}").unwrap();
        assert_eq!(patch.cost_tag, None);
    }
}

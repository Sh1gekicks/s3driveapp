//! 設定（06 §2）。
//!
//! [`Settings`] は IPC で受け渡すアプリ設定、[`SettingsFile`] は `settings.json` 全体
//! （アカウントごとの接続・認証情報の表示用情報を含む）を表す。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::common::{ConnectionId, StorageClass, Timestamp};
use super::connection::CostTag;
use super::search::Sort;

pub const SETTINGS_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Appearance {
    Auto,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct GeneralSettings {
    pub appearance: Appearance,
    /// null は `~/Downloads`。
    pub download_dir: Option<String>,
    pub show_hidden: bool,
    pub show_menu_bar_icon: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            appearance: Appearance::Auto,
            download_dir: None,
            show_hidden: false,
            show_menu_bar_icon: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ViewMode {
    List,
    Grid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct ViewSettings {
    pub mode: ViewMode,
    pub sort: Sort,
    pub inspector: bool,
}

impl Default for ViewSettings {
    fn default() -> Self {
        Self {
            mode: ViewMode::List,
            sort: Sort::default(),
            inspector: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct TransferSettings {
    pub max_files: u32,
    pub max_parts_per_file: u32,
    pub multipart_threshold_mb: u32,
    pub default_storage_class: StorageClass,
    pub normalize_nfc: bool,
    pub ignore: Vec<String>,
    pub notify_on_complete: bool,
}

impl Default for TransferSettings {
    fn default() -> Self {
        Self {
            max_files: 3,
            max_parts_per_file: 4,
            multipart_threshold_mb: 16,
            default_storage_class: StorageClass::Standard,
            normalize_nfc: true,
            ignore: vec![".DS_Store".to_string()],
            notify_on_complete: true,
        }
    }
}

impl TransferSettings {
    /// 範囲外の値を設計上の範囲に収める（03 §7）。
    pub fn clamped(&self) -> Self {
        let mut s = self.clone();
        s.max_files = s.max_files.clamp(1, 8);
        s.max_parts_per_file = s.max_parts_per_file.clamp(1, 16);
        s.multipart_threshold_mb = s.multipart_threshold_mb.clamp(8, 64);
        s
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct CostSettings {
    pub use_cost_explorer: bool,
}

impl Default for CostSettings {
    fn default() -> Self {
        Self {
            use_cost_explorer: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct SearchSettings {
    /// インデックスを自動更新するまでの時間（0 は手動のみ）。
    pub auto_refresh_minutes: u32,
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self {
            auto_refresh_minutes: 60,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LogLevel {
    Info,
    Debug,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AdvancedSettings {
    pub log_level: LogLevel,
    pub auto_check_update: bool,
}

impl Default for AdvancedSettings {
    fn default() -> Self {
        Self {
            log_level: LogLevel::Info,
            auto_check_update: true,
        }
    }
}

/// IPC で受け渡すアプリ設定。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Settings {
    pub general: GeneralSettings,
    pub view: ViewSettings,
    pub transfer: TransferSettings,
    pub cost: CostSettings,
    pub search: SearchSettings,
    pub advanced: AdvancedSettings,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Profile {
    pub email: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRecord {
    pub id: String,
    pub access_key_id_masked: String,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRecord {
    pub id: ConnectionId,
    pub bucket: String,
    pub region: String,
    pub credential_id: String,
    #[serde(default)]
    pub role_arn: Option<String>,
    #[serde(default)]
    pub external_id: Option<String>,
    #[serde(default)]
    pub use_source_identity: bool,
    #[serde(default)]
    pub cost_tag: Option<CostTag>,
    #[serde(default)]
    pub default_storage_class: Option<StorageClass>,
    /// エンドポイントの上書き（テスト用。開発ビルドでのみ有効）。
    #[serde(default)]
    pub endpoint_url: Option<String>,
}

/// 前回表示していた場所（03 §2）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Location {
    pub connection_id: ConnectionId,
    pub prefix: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Account {
    pub profile: Profile,
    pub credentials: Vec<CredentialRecord>,
    pub connections: Vec<ConnectionRecord>,
    pub last_location: Option<Location>,
}

/// `settings.json` 全体。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsFile {
    pub version: u32,
    #[serde(flatten)]
    pub settings: Settings,
    #[serde(default)]
    pub accounts: BTreeMap<String, Account>,
    /// 最後にサインインしていたアカウント（起動時のセッション復元に使う）。
    #[serde(default)]
    pub last_account: Option<String>,
}

impl Default for SettingsFile {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            settings: Settings::default(),
            accounts: BTreeMap::new(),
            last_account: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_design() {
        let s = Settings::default();
        assert_eq!(s.general.appearance, Appearance::Auto);
        assert!(!s.general.show_hidden);
        assert!(s.general.show_menu_bar_icon);
        assert_eq!(s.transfer.max_files, 3);
        assert_eq!(s.transfer.max_parts_per_file, 4);
        assert_eq!(s.transfer.multipart_threshold_mb, 16);
        assert_eq!(s.transfer.ignore, vec![".DS_Store"]);
        assert!(s.cost.use_cost_explorer);
        assert_eq!(s.search.auto_refresh_minutes, 60);
        assert_eq!(s.advanced.log_level, LogLevel::Info);
    }

    #[test]
    fn settings_file_reads_the_documented_shape() {
        let json = r#"{
          "version": 1,
          "general": { "appearance": "dark", "downloadDir": null, "showHidden": true, "showMenuBarIcon": true },
          "view": { "mode": "grid", "sort": { "key": "name", "dir": 1 }, "inspector": false },
          "accounts": {
            "123": {
              "profile": { "email": "a@example.com", "name": "A" },
              "credentials": [{ "id": "c1", "accessKeyIdMasked": "AKIA****1234", "createdAt": "2026-09-27T05:30:00Z" }],
              "connections": [{ "id": "k1", "bucket": "b", "region": "ap-northeast-1", "credentialId": "c1" }],
              "lastLocation": { "connectionId": "k1", "prefix": "a/" }
            }
          }
        }"#;
        let file: SettingsFile = serde_json::from_str(json).unwrap();
        assert_eq!(file.settings.general.appearance, Appearance::Dark);
        assert_eq!(file.settings.view.mode, ViewMode::Grid);
        assert_eq!(file.settings.transfer.max_files, 3);
        let account = &file.accounts["123"];
        assert_eq!(account.connections[0].bucket, "b");
        assert_eq!(account.last_location.as_ref().unwrap().prefix, "a/");
    }

    #[test]
    fn transfer_settings_are_clamped() {
        let s = TransferSettings {
            max_files: 99,
            max_parts_per_file: 0,
            multipart_threshold_mb: 1,
            ..TransferSettings::default()
        }
        .clamped();
        assert_eq!(
            (s.max_files, s.max_parts_per_file, s.multipart_threshold_mb),
            (8, 1, 8)
        );
    }
}

//! 秘密情報の保存先（06 §3）。本番は macOS キーチェーン、テストはメモリ上の実装を使う（08 §3）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::{CoreError, CoreResult};

/// キーチェーンのサービス名（開発ビルドは `.dev` を付けて本番と分ける。06 §1）。
pub const KEYCHAIN_SERVICE: &str = if cfg!(debug_assertions) {
    "io.github.sh1gekicks.s3drive.dev"
} else {
    "io.github.sh1gekicks.s3drive"
};

pub trait SecretStore: Send + Sync + 'static {
    fn get(&self, account: &str) -> CoreResult<Option<String>>;
    fn set(&self, account: &str, value: &str) -> CoreResult<()>;
    fn delete(&self, account: &str) -> CoreResult<()>;
}

/// キーチェーンの項目名。
pub fn google_account(sub: &str) -> String {
    format!("google:{sub}")
}

pub fn aws_account(credential_id: &str) -> String {
    format!("aws:{credential_id}")
}

/// メモリ上の実装（テスト・Linux 用）。
#[derive(Default)]
pub struct MemorySecretStore {
    items: Mutex<HashMap<String, String>>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemorySecretStore {
    fn get(&self, account: &str) -> CoreResult<Option<String>> {
        Ok(self.items.lock().unwrap().get(account).cloned())
    }

    fn set(&self, account: &str, value: &str) -> CoreResult<()> {
        self.items
            .lock()
            .unwrap()
            .insert(account.to_string(), value.to_string());
        Ok(())
    }

    fn delete(&self, account: &str) -> CoreResult<()> {
        self.items.lock().unwrap().remove(account);
        Ok(())
    }
}

/// keyring-core の資格情報ストアを使う実装。
pub struct KeyringSecretStore {
    service: String,
    store: Arc<keyring_core::api::CredentialStore>,
}

impl KeyringSecretStore {
    pub fn new(service: impl Into<String>, store: Arc<keyring_core::api::CredentialStore>) -> Self {
        Self {
            service: service.into(),
            store,
        }
    }

    /// ログインキーチェーンの汎用パスワード項目に保存する（keyring 4 系の `keychain` モジュール）。
    #[cfg(target_os = "macos")]
    pub fn macos_keychain() -> CoreResult<Self> {
        let store = apple_native_keyring_store::keychain::Store::new().map_err(map_err)?;
        Ok(Self::new(KEYCHAIN_SERVICE, store))
    }

    fn entry(&self, account: &str) -> CoreResult<keyring_core::Entry> {
        self.store
            .build(&self.service, account, None)
            .map_err(map_err)
    }
}

fn map_err(err: keyring_core::Error) -> CoreError {
    // 値は含めない（Error の表示に秘密情報は含まれない）
    CoreError::internal(format!("keychain: {err}"))
}

impl SecretStore for KeyringSecretStore {
    fn get(&self, account: &str) -> CoreResult<Option<String>> {
        match self.entry(account)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(map_err(e)),
        }
    }

    fn set(&self, account: &str, value: &str) -> CoreResult<()> {
        self.entry(account)?.set_password(value).map_err(map_err)
    }

    fn delete(&self, account: &str) -> CoreResult<()> {
        match self.entry(account)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(map_err(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exercise(store: &dyn SecretStore) {
        assert_eq!(store.get("aws:1").unwrap(), None);
        store.set("aws:1", "secret").unwrap();
        assert_eq!(store.get("aws:1").unwrap().as_deref(), Some("secret"));
        store.set("aws:1", "rotated").unwrap();
        assert_eq!(store.get("aws:1").unwrap().as_deref(), Some("rotated"));
        store.delete("aws:1").unwrap();
        assert_eq!(store.get("aws:1").unwrap(), None);
        store.delete("aws:1").unwrap();
    }

    #[test]
    fn memory_store_round_trips() {
        exercise(&MemorySecretStore::new());
    }

    #[test]
    fn keyring_store_round_trips_with_mock_backend() {
        let mock = keyring_core::mock::Store::new().unwrap();
        exercise(&KeyringSecretStore::new("test", mock));
    }
}

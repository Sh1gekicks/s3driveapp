//! アプリの状態（05 §1.3）。

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use s3drive_core::auth::google::{GoogleAuth, GoogleConfig};
use s3drive_core::auth::{AuthProvider, FixedAuth, UnconfiguredAuth};
use s3drive_core::store::SecretStore;
use s3drive_core::{Core, CoreConfig};
use tauri::{App, Manager};

pub struct AppState {
    pub core: Core,
    /// 終了の確認を済ませた（再度の確認を避ける）。
    pub quitting: AtomicBool,
}

/// 秘密情報の保存先。macOS はキーチェーン、それ以外（開発時の Linux など）はメモリ上に置く。
fn secret_store() -> Result<Arc<dyn SecretStore>, Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    {
        Ok(Arc::new(
            s3drive_core::store::KeyringSecretStore::macos_keychain()?,
        ))
    }
    #[cfg(not(target_os = "macos"))]
    {
        log::warn!(
            "この OS ではキーチェーンを使えないため、認証情報をメモリ上に保持します（開発用）"
        );
        Ok(Arc::new(s3drive_core::store::MemorySecretStore::new()))
    }
}

/// 認証の実装。E2E ビルドと、OAuth クライアント未設定の開発ビルドは固定のセッションにする（09 §2.5）。
fn auth_provider() -> Result<Arc<dyn AuthProvider>, Box<dyn std::error::Error>> {
    if cfg!(feature = "e2e") {
        return Ok(Arc::new(FixedAuth::test_user()));
    }
    match GoogleConfig::from_build_env() {
        Some(config) => Ok(Arc::new(GoogleAuth::new(config)?)),
        None if cfg!(debug_assertions) => {
            log::warn!(
                "S3DRIVE_GOOGLE_CLIENT_ID が未設定のため、開発用の固定セッションでサインインします"
            );
            Ok(Arc::new(FixedAuth::test_user()))
        }
        None => Ok(Arc::new(UnconfiguredAuth)),
    }
}

pub fn init(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let mut data_dir = app.path().app_data_dir()?;
    if cfg!(debug_assertions)
        && let Some(name) = data_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
    {
        // 開発ビルドは保存先を本番と分ける（06 §1）
        data_dir.set_file_name(format!("{name}.dev"));
    }
    // テスト用のエンドポイント（moto）は開発ビルドと E2E ビルドでのみ有効にする
    let dev = cfg!(debug_assertions) || cfg!(feature = "e2e");
    let endpoint_override = dev
        .then(|| std::env::var("S3DRIVE_TEST_ENDPOINT").ok())
        .flatten()
        .filter(|s| !s.is_empty());
    let core = Core::new(CoreConfig {
        data_dir,
        secrets: secret_store()?,
        auth: auth_provider()?,
        endpoint_override,
        allow_connection_endpoints: dev,
    })?;

    let settings = core.settings_store().settings();
    log::set_max_level(match settings.advanced.log_level {
        s3drive_core::model::LogLevel::Debug => log::LevelFilter::Debug,
        s3drive_core::model::LogLevel::Info => log::LevelFilter::Info,
    });
    core.transfers().set_max_files(settings.transfer.max_files);

    app.manage(AppState {
        core,
        quitting: AtomicBool::new(false),
    });
    Ok(())
}

//! 設定・アプリ（05 §3.8）。

use std::sync::atomic::Ordering;
use std::time::Duration;

use s3drive_core::model::{Appearance, MenuState, Settings, StartupInfo, UpdateEvent, UpdateInfo};
use s3drive_core::{CoreError, ErrorCode};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::UpdaterExt;

use super::{CmdResult, ipc};
use crate::events::{SETTINGS_CHANGED, emit_all};
use crate::state::AppState;

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> Settings {
    state.core.settings_store().settings()
}

/// 設定の変更を保存し、全ウィンドウに知らせる。関連するネイティブの状態も更新する。
pub fn apply_settings(app: &AppHandle, settings: &Settings) {
    let state = app.state::<AppState>();
    state
        .core
        .transfers()
        .set_max_files(settings.transfer.max_files);
    log::set_max_level(match settings.advanced.log_level {
        s3drive_core::model::LogLevel::Debug => log::LevelFilter::Debug,
        s3drive_core::model::LogLevel::Info => log::LevelFilter::Info,
    });
    crate::window::apply_theme(app, settings.general.appearance);
    crate::tray::set_visible(app, settings.general.show_menu_bar_icon);
    emit_all(app, SETTINGS_CHANGED, settings.clone());
}

#[tauri::command]
pub fn settings_update(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: serde_json::Value,
) -> CmdResult<Settings> {
    // ダウンロード先などのパスはフロントエンドから受け取らない（05 §3.9）
    let settings = ipc(state.core.settings_store().patch_from_ipc(patch))?;
    apply_settings(&app, &settings);
    Ok(settings)
}

/// 全ウィンドウの外観を切り替える（02 §7.2）。
#[tauri::command]
pub fn app_set_theme(
    app: AppHandle,
    state: State<'_, AppState>,
    theme: Appearance,
) -> CmdResult<()> {
    let settings = ipc(state
        .core
        .settings_store()
        .patch(serde_json::json!({ "general": { "appearance": theme } })))?;
    apply_settings(&app, &settings);
    Ok(())
}

/// ダウンロード先の選択（設定「変更…」）。パスはフロントエンドから受け取らない。
#[tauri::command]
pub async fn app_choose_download_dir(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<Settings> {
    let dialog = app.dialog().file().set_title("ダウンロード先を選択");
    let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_folder())
        .await
        .map_err(|e| CoreError::internal(e.to_string()))?;
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else {
        return Ok(state.core.settings_store().settings());
    };
    let settings = ipc(state.core.settings_store().set_download_dir(&path))?;
    apply_settings(&app, &settings);
    Ok(settings)
}

#[tauri::command]
pub fn app_open_settings(app: AppHandle) -> CmdResult<()> {
    crate::window::open_settings(&app).map_err(|e| CoreError::internal(e.to_string()).into())
}

#[tauri::command]
pub fn app_startup_info(app: AppHandle, state: State<'_, AppState>) -> StartupInfo {
    StartupInfo {
        session: state.core.session(),
        db_recreated: state.core.db_recreated(),
        version: app.package_info().version.to_string(),
    }
}

/// メニュー項目の有効・無効を更新する（03 §9.4）。
#[tauri::command]
pub fn menu_update_state(app: AppHandle, state: MenuState) {
    crate::menu::update(&app, &state);
}

#[tauri::command]
pub fn app_open_logs(app: AppHandle) -> CmdResult<()> {
    let dir = app
        .path()
        .app_log_dir()
        .map_err(|e| CoreError::internal(e.to_string()))?;
    std::fs::create_dir_all(&dir).map_err(|e| CoreError::local_io(&dir, &e))?;
    app.opener()
        .open_path(dir.display().to_string(), None::<&str>)
        .map_err(|e| CoreError::internal(format!("open logs: {e}")).into())
}

#[tauri::command]
pub async fn app_clear_cache(state: State<'_, AppState>) -> CmdResult<()> {
    ipc(state.core.clear_metrics_cache().await)
}

#[tauri::command]
pub async fn app_check_update(app: AppHandle) -> CmdResult<Option<UpdateInfo>> {
    let updater = app.updater().map_err(|e| {
        CoreError::with_message(ErrorCode::Network, "アップデートを確認できませんでした")
            .detail(e.to_string())
    })?;
    let update = updater.check().await.map_err(|e| {
        CoreError::with_message(ErrorCode::Network, "アップデートを確認できませんでした")
            .detail(e.to_string())
    })?;
    Ok(update.map(|u| UpdateInfo {
        version: u.version.clone(),
        notes: u.body.clone(),
    }))
}

/// 更新をダウンロードし、署名を検証してから適用して再起動する（08 §7）。
///
/// 転送中の場合はフロントエンドが確認し、`when_idle` で結果を渡す。真なら転送が終わるのを待ってから再起動し
/// （待つ間にコマンドは戻る）、偽なら転送を中止してから再起動する。
#[tauri::command]
pub async fn app_install_update(
    app: AppHandle,
    state: State<'_, AppState>,
    on_event: Channel<UpdateEvent>,
    when_idle: Option<bool>,
) -> CmdResult<()> {
    // インストール済みで再起動を待っている場合は、ダウンロードし直さない
    if !state.restart_pending.load(Ordering::Relaxed) {
        download_and_install(&app, on_event).await?;
        state.restart_pending.store(true, Ordering::Relaxed);
    }
    if when_idle.unwrap_or(false) && state.core.transfers().summary().0 > 0 {
        let core = state.core.clone();
        tauri::async_runtime::spawn(async move {
            core.transfers().wait_idle().await;
            restart(&app).await;
        });
        return Ok(());
    }
    restart(&app).await;
    Ok(())
}

async fn download_and_install(app: &AppHandle, on_event: Channel<UpdateEvent>) -> CmdResult<()> {
    let update = app
        .updater()
        .map_err(|e| CoreError::internal(e.to_string()))?
        .check()
        .await
        .map_err(|e| {
            CoreError::with_message(ErrorCode::Network, "アップデートを確認できませんでした")
                .detail(e.to_string())
        })?
        .ok_or_else(|| {
            CoreError::with_message(ErrorCode::NotFound, "新しいバージョンはありません")
        })?;
    let mut downloaded = 0u64;
    let progress = on_event.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress.send(UpdateEvent::Progress { downloaded, total });
            },
            move || {
                let _ = on_event.send(UpdateEvent::Installed);
            },
        )
        .await
        .map_err(|e| {
            CoreError::with_message(ErrorCode::Network, "アップデートを適用できませんでした")
                .detail(e.to_string())
        })?;
    Ok(())
}

/// 残っている転送を中止してから（終了と同じく 3 秒を上限に中止処理を行う。04 §14.4）再起動する。
async fn restart(app: &AppHandle) {
    let state = app.state::<AppState>();
    state.quitting.store(true, Ordering::Relaxed);
    state
        .core
        .transfers()
        .shutdown(Duration::from_secs(3))
        .await;
    app.restart();
}

/// ウィンドウが前面にないときだけ macOS の通知を出す（03 §11）。
#[tauri::command]
pub fn app_notify(app: AppHandle, state: State<'_, AppState>, title: String, body: String) {
    if !state
        .core
        .settings_store()
        .settings()
        .transfer
        .notify_on_complete
    {
        return;
    }
    let focused = app
        .get_webview_window("main")
        .and_then(|w| w.is_focused().ok())
        .unwrap_or(false);
    if !focused && let Err(e) = app.notification().builder().title(title).body(body).show() {
        log::debug!("通知を表示できません: {e}");
    }
}

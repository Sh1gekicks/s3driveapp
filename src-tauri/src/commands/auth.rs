//! 認証（05 §3.1）。

use s3drive_core::model::UserSession;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use super::{CmdResult, ipc};
use crate::events::{SESSION_CHANGED, emit_all};
use crate::state::AppState;

#[tauri::command]
pub fn auth_get_session(state: State<'_, AppState>) -> Option<UserSession> {
    state.core.session()
}

/// サインイン後の後始末（アプリが中断したマルチパートアップロードの中止。04 §14.5）。
fn after_sign_in(state: &AppState) {
    let core = state.core.clone();
    tauri::async_runtime::spawn(async move {
        match core.abort_stale_uploads().await {
            Ok(0) => {}
            Ok(n) => log::info!("中断していた {n} 件のアップロードを中止しました"),
            Err(e) => log::warn!("中断したアップロードを中止できません: {e}"),
        }
    });
}

#[tauri::command]
pub async fn auth_restore(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<Option<UserSession>> {
    let session = ipc(state.core.auth_restore().await)?;
    if session.is_some() {
        after_sign_in(&state);
    }
    emit_all(&app, SESSION_CHANGED, session.clone());
    Ok(session)
}

#[tauri::command]
pub async fn auth_sign_in(app: AppHandle, state: State<'_, AppState>) -> CmdResult<UserSession> {
    let opener = app.clone();
    let open = move |url: &str| {
        opener
            .opener()
            .open_url(url, None::<&str>)
            .map_err(|e| s3drive_core::CoreError::internal(format!("open url: {e}")))
    };
    let session = ipc(state.core.auth_sign_in(&open).await)?;
    after_sign_in(&state);
    emit_all(&app, SESSION_CHANGED, Some(session.clone()));
    crate::window::show_main(&app);
    Ok(session)
}

#[tauri::command]
pub fn auth_cancel_sign_in(state: State<'_, AppState>) {
    state.core.auth_cancel_sign_in();
}

#[tauri::command]
pub async fn auth_sign_out(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    ipc(state.core.auth_sign_out().await)?;
    emit_all(&app, SESSION_CHANGED, None::<UserSession>);
    Ok(())
}

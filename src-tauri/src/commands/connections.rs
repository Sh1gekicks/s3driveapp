//! 接続・認証情報（05 §3.2）。

use s3drive_core::model::{
    BucketInfo, Connection, ConnectionInput, ConnectionPatch, ConnectionTestResult,
    CredentialSummary, Location,
};
use tauri::{AppHandle, State};

use super::{CmdResult, ipc};
use crate::events::{CONNECTIONS_CHANGED, emit_all};
use crate::state::AppState;

#[tauri::command]
pub fn connection_list(state: State<'_, AppState>) -> CmdResult<Vec<Connection>> {
    ipc(state.core.connections().list())
}

#[tauri::command]
pub async fn connection_test(
    state: State<'_, AppState>,
    input: ConnectionInput,
) -> CmdResult<ConnectionTestResult> {
    ipc(state.core.connections().test(&input).await)
}

#[tauri::command]
pub async fn connection_create(
    app: AppHandle,
    state: State<'_, AppState>,
    input: ConnectionInput,
) -> CmdResult<Connection> {
    let c = ipc(state.core.connections().create(input).await)?;
    emit_all(&app, CONNECTIONS_CHANGED, ());
    Ok(c)
}

#[tauri::command]
pub async fn connection_update(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    input: ConnectionInput,
) -> CmdResult<Connection> {
    let c = ipc(state.core.connections().update(&id, input).await)?;
    emit_all(&app, CONNECTIONS_CHANGED, ());
    Ok(c)
}

#[tauri::command]
pub fn connection_patch(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    patch: ConnectionPatch,
) -> CmdResult<Connection> {
    let c = ipc(state.core.connections().patch(&id, patch))?;
    emit_all(&app, CONNECTIONS_CHANGED, ());
    Ok(c)
}

#[tauri::command]
pub async fn connection_delete(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<()> {
    ipc(state.core.connections().delete(&id).await)?;
    emit_all(&app, CONNECTIONS_CHANGED, ());
    Ok(())
}

#[tauri::command]
pub fn connection_reorder(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> CmdResult<()> {
    ipc(state.core.connections().reorder(&ids))?;
    emit_all(&app, CONNECTIONS_CHANGED, ());
    Ok(())
}

/// 前回表示していた場所（起動時に復元する。03 §2）。
#[tauri::command]
pub fn connection_last_location(state: State<'_, AppState>) -> CmdResult<Option<Location>> {
    ipc(state.core.connections().last_location())
}

#[tauri::command]
pub fn connection_set_location(state: State<'_, AppState>, location: Location) -> CmdResult<()> {
    ipc(state.core.connections().set_last_location(location))
}

#[tauri::command]
pub fn credential_list(state: State<'_, AppState>) -> CmdResult<Vec<CredentialSummary>> {
    ipc(state.core.connections().credential_list())
}

#[tauri::command]
pub async fn credential_update(
    app: AppHandle,
    state: State<'_, AppState>,
    credential_id: String,
    access_key_id: String,
    secret_access_key: String,
) -> CmdResult<()> {
    ipc(state
        .core
        .connections()
        .credential_update(&credential_id, &access_key_id, &secret_access_key)
        .await)?;
    emit_all(&app, CONNECTIONS_CHANGED, ());
    Ok(())
}

#[tauri::command]
pub async fn bucket_get_info(
    state: State<'_, AppState>,
    connection_id: String,
    force: Option<bool>,
) -> CmdResult<BucketInfo> {
    ipc(state
        .core
        .connections()
        .bucket_info(&connection_id, force.unwrap_or(false))
        .await)
}

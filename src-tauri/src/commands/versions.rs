//! バージョン（05 §3.4）。

use s3drive_core::model::{BatchResult, ObjectVersion, VersionPage};
use tauri::State;

use super::{CmdResult, ipc};
use crate::state::AppState;

#[tauri::command]
pub async fn versions_list(
    state: State<'_, AppState>,
    connection_id: String,
    key: String,
    cursor: Option<String>,
) -> CmdResult<VersionPage> {
    ipc(state
        .core
        .versions()
        .list(&connection_id, &key, cursor)
        .await)
}

#[tauri::command]
pub async fn version_restore(
    state: State<'_, AppState>,
    connection_id: String,
    key: String,
    version_id: String,
) -> CmdResult<ObjectVersion> {
    ipc(state
        .core
        .versions()
        .restore(&connection_id, &key, &version_id)
        .await)
}

#[tauri::command]
pub async fn version_delete(
    state: State<'_, AppState>,
    connection_id: String,
    key: String,
    version_id: String,
) -> CmdResult<()> {
    ipc(state
        .core
        .versions()
        .delete(&connection_id, &key, &version_id)
        .await)
}

#[tauri::command]
pub async fn deleted_restore(
    state: State<'_, AppState>,
    connection_id: String,
    keys: Vec<String>,
) -> CmdResult<BatchResult> {
    ipc(state.core.versions().undelete(&connection_id, &keys).await)
}

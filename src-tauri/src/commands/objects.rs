//! オブジェクト・フォルダ（05 §3.3）。

use std::sync::Arc;

use s3drive_core::model::{
    BatchEvent, BatchResult, Decisions, Entry, FolderSummary, JobId, ListOptions, ListPage,
    ObjectDetail, RemoteConflict, RestoreTier, StorageClass, Target,
};
use tauri::State;
use tauri::ipc::Channel;

use super::{CmdResult, ipc};
use crate::events::ChannelSink;
use crate::state::AppState;

#[tauri::command]
pub async fn objects_list_page(
    state: State<'_, AppState>,
    connection_id: String,
    prefix: String,
    token: Option<String>,
    opts: ListOptions,
) -> CmdResult<ListPage> {
    ipc(state
        .core
        .objects()
        .list_page(&connection_id, &prefix, token, opts)
        .await)
}

#[tauri::command]
pub async fn object_head(
    state: State<'_, AppState>,
    connection_id: String,
    key: String,
    version_id: Option<String>,
) -> CmdResult<ObjectDetail> {
    ipc(state
        .core
        .objects()
        .head(&connection_id, &key, version_id.as_deref())
        .await)
}

#[tauri::command]
pub async fn folder_create(
    state: State<'_, AppState>,
    connection_id: String,
    prefix: String,
    name: String,
) -> CmdResult<Entry> {
    ipc(state
        .core
        .objects()
        .create_folder(&connection_id, &prefix, &name)
        .await)
}

#[tauri::command]
pub async fn folder_summary(
    state: State<'_, AppState>,
    connection_id: String,
    prefix: String,
) -> CmdResult<FolderSummary> {
    ipc(state
        .core
        .objects()
        .folder_summary(&connection_id, &prefix)
        .await)
}

#[tauri::command]
pub async fn folder_children(
    state: State<'_, AppState>,
    connection_id: String,
    prefix: String,
) -> CmdResult<Vec<Entry>> {
    ipc(state
        .core
        .objects()
        .folder_children(&connection_id, &prefix)
        .await)
}

#[tauri::command]
pub async fn objects_find_conflicts(
    state: State<'_, AppState>,
    connection_id: String,
    targets: Vec<Target>,
    dest_prefix: String,
) -> CmdResult<Vec<RemoteConflict>> {
    ipc(state
        .core
        .objects()
        .find_conflicts(&connection_id, &targets, &dest_prefix)
        .await)
}

#[tauri::command]
pub async fn objects_delete(
    state: State<'_, AppState>,
    connection_id: String,
    targets: Vec<Target>,
    all_versions: bool,
    on_event: Channel<BatchEvent>,
) -> CmdResult<JobId> {
    ipc(state
        .core
        .objects()
        .delete(
            &connection_id,
            targets,
            all_versions,
            Arc::new(ChannelSink(on_event)),
        )
        .await)
}

#[tauri::command]
pub async fn objects_move(
    state: State<'_, AppState>,
    connection_id: String,
    targets: Vec<Target>,
    dest_prefix: String,
    decisions: Option<Decisions>,
    on_event: Channel<BatchEvent>,
) -> CmdResult<JobId> {
    ipc(state
        .core
        .objects()
        .move_objects(
            &connection_id,
            targets,
            dest_prefix,
            decisions.unwrap_or_default(),
            Arc::new(ChannelSink(on_event)),
        )
        .await)
}

#[tauri::command]
pub async fn object_rename(
    state: State<'_, AppState>,
    connection_id: String,
    target: Target,
    new_name: String,
    on_event: Channel<BatchEvent>,
) -> CmdResult<JobId> {
    ipc(state
        .core
        .objects()
        .rename(
            &connection_id,
            target,
            &new_name,
            Arc::new(ChannelSink(on_event)),
        )
        .await)
}

#[tauri::command]
pub async fn objects_change_storage_class(
    state: State<'_, AppState>,
    connection_id: String,
    targets: Vec<Target>,
    storage_class: StorageClass,
    on_event: Channel<BatchEvent>,
) -> CmdResult<JobId> {
    ipc(state
        .core
        .objects()
        .change_storage_class(
            &connection_id,
            targets,
            storage_class,
            Arc::new(ChannelSink(on_event)),
        )
        .await)
}

#[tauri::command]
pub async fn objects_request_restore(
    state: State<'_, AppState>,
    connection_id: String,
    targets: Vec<Target>,
    tier: RestoreTier,
    days: Option<u32>,
) -> CmdResult<BatchResult> {
    ipc(state
        .core
        .objects()
        .request_restore(&connection_id, &targets, tier, days)
        .await)
}

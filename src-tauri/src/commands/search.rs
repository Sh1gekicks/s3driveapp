//! 検索（05 §3.6）。

use std::sync::Arc;

use s3drive_core::model::{IndexEvent, IndexStatus, JobId, SearchQuery, SearchResult};
use tauri::State;
use tauri::ipc::Channel;

use super::{CmdResult, ipc};
use crate::events::ChannelSink;
use crate::state::AppState;

#[tauri::command]
pub async fn search_query(
    state: State<'_, AppState>,
    connection_id: String,
    query: SearchQuery,
) -> CmdResult<SearchResult> {
    ipc(state.core.search().query(&connection_id, query).await)
}

#[tauri::command]
pub async fn search_index_status(
    state: State<'_, AppState>,
    connection_id: String,
) -> CmdResult<IndexStatus> {
    ipc(state.core.search().status(&connection_id).await)
}

#[tauri::command]
pub async fn search_index_rebuild(
    state: State<'_, AppState>,
    connection_id: String,
    on_event: Channel<IndexEvent>,
) -> CmdResult<JobId> {
    ipc(state
        .core
        .search()
        .rebuild(&connection_id, Arc::new(ChannelSink(on_event)))
        .await)
}

#[tauri::command]
pub async fn search_index_delete(
    state: State<'_, AppState>,
    connection_id: String,
) -> CmdResult<()> {
    ipc(state.core.search().delete(&connection_id).await)
}

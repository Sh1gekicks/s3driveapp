//! 転送（05 §3.5）。ローカルパスはフロントエンドから受け取らず、選択 ID だけを扱う（05 §3.9）。

use std::sync::Arc;

use s3drive_core::model::{
    Decisions, DownloadDestination, JobId, Selection, Target, TransferEvent, TransferJob,
    UploadPlan,
};
use s3drive_core::{CoreError, ErrorCode};
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use super::{CmdResult, ipc};
use crate::events::ChannelSink;
use crate::state::AppState;

#[tauri::command]
pub fn transfer_subscribe(
    state: State<'_, AppState>,
    on_event: Channel<TransferEvent>,
) -> Vec<TransferJob> {
    state
        .core
        .transfers()
        .subscribe(Arc::new(ChannelSink(on_event)))
}

/// Rust 側でファイル（またはフォルダ）の選択ダイアログを開く。
#[tauri::command]
pub async fn pick_upload_files(
    app: AppHandle,
    state: State<'_, AppState>,
    directories: bool,
) -> CmdResult<Option<Selection>> {
    let dialog = app.dialog().file().set_title(if directories {
        "アップロードするフォルダを選択"
    } else {
        "アップロードするファイルを選択"
    });
    let picked = tauri::async_runtime::spawn_blocking(move || {
        if directories {
            dialog.blocking_pick_folders()
        } else {
            dialog.blocking_pick_files()
        }
    })
    .await
    .map_err(|e| CoreError::internal(e.to_string()))?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let paths: Vec<_> = picked
        .into_iter()
        .filter_map(|p| p.into_path().ok())
        .collect();
    if paths.is_empty() {
        return Ok(None);
    }
    Ok(Some(state.core.selections().register(paths)))
}

#[tauri::command]
pub async fn upload_prepare(
    state: State<'_, AppState>,
    connection_id: String,
    prefix: String,
    selection_id: String,
) -> CmdResult<UploadPlan> {
    ipc(state
        .core
        .upload_prepare(&connection_id, &prefix, &selection_id)
        .await)
}

#[tauri::command]
pub async fn upload_start(
    state: State<'_, AppState>,
    plan_id: String,
    decisions: Option<Decisions>,
) -> CmdResult<JobId> {
    ipc(state
        .core
        .upload_start(&plan_id, decisions.unwrap_or_default())
        .await)
}

/// 保存先フォルダの選択ダイアログ（⇧⌘D「場所を指定してダウンロード…」）。
#[tauri::command]
pub async fn pick_download_dir(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<Option<Selection>> {
    let dialog = app.dialog().file().set_title("保存先を選択");
    let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_folder())
        .await
        .map_err(|e| CoreError::internal(e.to_string()))?;
    Ok(picked
        .and_then(|p| p.into_path().ok())
        .map(|p| state.core.selections().register(vec![p])))
}

#[tauri::command]
pub async fn download_start(
    state: State<'_, AppState>,
    connection_id: String,
    targets: Vec<Target>,
    destination: DownloadDestination,
) -> CmdResult<JobId> {
    ipc(state
        .core
        .download_start(&connection_id, targets, destination)
        .await)
}

/// 保存したファイルを Finder で表示する。
#[tauri::command]
pub fn download_reveal(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
) -> CmdResult<()> {
    let paths = state.core.transfers().saved_paths(&job_id);
    let first = paths.first().ok_or_else(|| {
        CoreError::with_message(ErrorCode::NotFound, "保存したファイルが見つかりません")
    })?;
    app.opener()
        .reveal_item_in_dir(first)
        .map_err(|e| CoreError::internal(format!("reveal: {e}")).into())
}

#[tauri::command]
pub fn job_cancel(state: State<'_, AppState>, job_id: String) {
    state.core.job_cancel(&job_id);
}

#[tauri::command]
pub fn transfer_retry(state: State<'_, AppState>, job_id: String) -> CmdResult<JobId> {
    ipc(state.core.transfer_retry(&job_id))
}

#[tauri::command]
pub fn transfer_clear_finished(state: State<'_, AppState>) {
    state.core.transfers().clear_finished();
}

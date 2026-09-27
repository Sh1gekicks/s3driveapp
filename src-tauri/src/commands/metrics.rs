//! メトリクス・コスト（05 §3.7）。

use s3drive_core::model::{CostSummary, PriceTable, StorageMetrics};
use tauri::State;

use super::{CmdResult, ipc};
use crate::state::AppState;

#[tauri::command]
pub async fn metrics_storage(
    state: State<'_, AppState>,
    connection_id: String,
    force: bool,
) -> CmdResult<StorageMetrics> {
    ipc(state.core.metrics_storage(&connection_id, force).await)
}

/// 保存済みの結果を返す。Cost Explorer には問い合わせない（04 §13.4）。
#[tauri::command]
pub async fn cost_summary(
    state: State<'_, AppState>,
    connection_id: String,
) -> CmdResult<Option<CostSummary>> {
    ipc(state.core.cost_summary(&connection_id).await)
}

/// Cost Explorer に問い合わせる。ダッシュボードの「更新」「取得」からのみ呼ぶ。
#[tauri::command]
pub async fn cost_refresh(
    state: State<'_, AppState>,
    connection_id: String,
) -> CmdResult<CostSummary> {
    ipc(state.core.cost_refresh(&connection_id).await)
}

#[tauri::command]
pub async fn pricing_get(state: State<'_, AppState>, region: String) -> CmdResult<PriceTable> {
    ipc(state.core.pricing_get(&region).await)
}

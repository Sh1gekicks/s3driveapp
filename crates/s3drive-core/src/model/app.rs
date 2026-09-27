//! アプリ（メニュー・イベント・自動更新）の IPC の型（05 §3.8、05 §4）。

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::common::ConnectionId;
use super::connection::UserSession;
use super::search::IndexStatus;

/// 表示中の画面（01 §5.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AppView {
    Signin,
    Files,
    Dashboard,
}

/// アプリのメニューバーの有効・無効を決める状態（03 §9.4）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MenuState {
    pub view: AppView,
    /// 選択している項目の数。
    pub selection_count: u32,
    /// 選択に取り出していないアーカイブを含む。
    pub has_archived: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub can_go_up: bool,
    pub inspector_visible: bool,
    pub filters_visible: bool,
    pub show_hidden: bool,
    pub show_deleted: bool,
    /// 表示モード（`list`／`grid`）。
    pub view_mode: String,
}

/// 更新の情報（`app_check_update`）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

/// 更新のダウンロードの進捗（`app_install_update` のチャネル）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
#[ts(export)]
pub enum UpdateEvent {
    #[serde(rename_all = "camelCase")]
    Progress {
        downloaded: u64,
        total: Option<u64>,
    },
    Installed,
}

/// メニューが選ばれた（`menu://action`）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MenuAction {
    pub id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DropPosition {
    pub x: f64,
    pub y: f64,
}

/// Finder からのドラッグ中（`dragdrop://enter`／`over`／`leave`）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DragPayload {
    pub position: Option<DropPosition>,
    pub names: Vec<String>,
}

/// ドロップされた（`dragdrop://drop`）。パスは Rust 側で保管済み。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DropPayload {
    pub selection_id: String,
    pub position: DropPosition,
    pub names: Vec<String>,
}

/// アーカイブの取り出し完了（`restore://completed`）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RestoreCompleted {
    pub connection_id: ConnectionId,
    pub key: String,
}

/// 背景でのインデックス更新の完了（`index://updated`）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IndexUpdated {
    pub connection_id: ConnectionId,
    pub status: IndexStatus,
}

/// 起動時に一度だけ知らせること（SQLite の作り直しなど）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StartupInfo {
    pub session: Option<UserSession>,
    pub db_recreated: bool,
    pub version: String,
}

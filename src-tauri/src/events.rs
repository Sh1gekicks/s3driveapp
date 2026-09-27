//! チャネルとグローバルイベント（05 §4）。

use s3drive_core::jobs::ProgressSink;
use serde::Serialize;
use tauri::ipc::{Channel, IpcResponse};
use tauri::{AppHandle, Emitter};

pub const SESSION_CHANGED: &str = "session://changed";
pub const SETTINGS_CHANGED: &str = "settings://changed";
pub const CONNECTIONS_CHANGED: &str = "connections://changed";
pub const MENU_ACTION: &str = "menu://action";
pub const DRAG_ENTER: &str = "dragdrop://enter";
pub const DRAG_OVER: &str = "dragdrop://over";
pub const DRAG_LEAVE: &str = "dragdrop://leave";
pub const DRAG_DROP: &str = "dragdrop://drop";
pub const RESTORE_COMPLETED: &str = "restore://completed";
pub const INDEX_UPDATED: &str = "index://updated";
pub const UPDATE_AVAILABLE: &str = "update://available";

/// コアの進捗通知を `Channel` で送る（05 §1.1）。
pub struct ChannelSink<E: IpcResponse>(pub Channel<E>);

impl<E> ProgressSink<E> for ChannelSink<E>
where
    E: IpcResponse + Send + Sync + 'static,
{
    fn send(&self, event: E) {
        // 受信側が閉じていても処理は継続する
        let _ = self.0.send(event);
    }
}

/// 全ウィンドウにイベントを送る。
pub fn emit_all<S: Serialize + Clone>(app: &AppHandle, event: &str, payload: S) {
    if let Err(e) = app.emit(event, payload) {
        log::warn!("イベントを送れません（{event}）: {e}");
    }
}

//! ウィンドウの生成・テーマ・閉じる挙動・終了の確認（01 §3、02 §7）。

use std::sync::atomic::Ordering;
use std::time::Duration;

use s3drive_core::model::{Appearance, DragPayload, DropPayload, DropPosition};
use tauri::{
    App, AppHandle, DragDropEvent, Emitter, Manager, RunEvent, Theme, WebviewUrl,
    WebviewWindowBuilder, Window, WindowEvent,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::events::{DRAG_DROP, DRAG_ENTER, DRAG_LEAVE, DRAG_OVER};
use crate::state::AppState;

pub const MAIN: &str = "main";
pub const SETTINGS: &str = "settings";

fn native_theme(appearance: Appearance) -> Option<Theme> {
    match appearance {
        Appearance::Auto => None,
        Appearance::Light => Some(Theme::Light),
        Appearance::Dark => Some(Theme::Dark),
    }
}

/// 起動時のちらつきを防ぐため、手動指定の外観を最初の描画より前に `data-theme` として設定する（02 §7.2）。
fn theme_script(appearance: Appearance) -> String {
    match appearance {
        Appearance::Auto => String::new(),
        Appearance::Light => "document.documentElement.setAttribute('data-theme','light');".into(),
        Appearance::Dark => "document.documentElement.setAttribute('data-theme','dark');".into(),
    }
}

fn appearance(app: &AppHandle) -> Appearance {
    app.state::<AppState>()
        .core
        .settings_store()
        .settings()
        .general
        .appearance
}

/// メインウィンドウを作る。`visible: false` で作り、最初の描画後にフロントエンドが表示する（02 §7.4）。
pub fn setup_main(app: &App) -> tauri::Result<()> {
    let handle = app.handle();
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == MAIN)
        .cloned()
        .expect("main window config");
    let appearance = appearance(handle);
    WebviewWindowBuilder::from_config(handle, &config)?
        .initialization_script(theme_script(appearance))
        .theme(native_theme(appearance))
        .build()?;
    Ok(())
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// 設定ウィンドウ（560×480、サイズ変更不可。03 §7）。開いていれば前面に出す。
pub fn open_settings(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(SETTINGS) {
        w.show()?;
        return w.set_focus();
    }
    let appearance = appearance(app);
    WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App("settings.html".into()))
        .title("設定")
        .inner_size(560.0, 480.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .visible(false)
        .initialization_script(theme_script(appearance))
        .theme(native_theme(appearance))
        .build()?;
    Ok(())
}

/// 全ウィンドウの外観を切り替える。「自動」はテーマの指定を外して OS に追従させる。
pub fn apply_theme(app: &AppHandle, appearance: Appearance) {
    for w in app.webview_windows().values() {
        let _ = w.set_theme(native_theme(appearance));
    }
}

fn names(paths: &[std::path::PathBuf]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect()
}

pub fn on_window_event(window: &Window, event: &WindowEvent) {
    match event {
        // メインウィンドウは閉じても破棄せず隠す。転送は継続する（01 §3）
        WindowEvent::CloseRequested { api, .. } if window.label() == MAIN => {
            api.prevent_close();
            let _ = window.hide();
        }
        // Finder からのドロップ。パスは選択 ID として保管し、フロントエンドには名前だけを渡す（05 §3.9）
        WindowEvent::DragDrop(drag) if window.label() == MAIN => {
            let scale = window.scale_factor().unwrap_or(1.0);
            let pos = |p: &tauri::PhysicalPosition<f64>| {
                let l = p.to_logical::<f64>(scale);
                DropPosition { x: l.x, y: l.y }
            };
            let result = match drag {
                DragDropEvent::Enter { paths, position } => window.emit(
                    DRAG_ENTER,
                    DragPayload {
                        position: Some(pos(position)),
                        names: names(paths),
                    },
                ),
                DragDropEvent::Over { position } => window.emit(
                    DRAG_OVER,
                    DragPayload {
                        position: Some(pos(position)),
                        names: Vec::new(),
                    },
                ),
                DragDropEvent::Drop { paths, position } => {
                    let state = window.state::<AppState>();
                    let selection = state.core.selections().register(paths.clone());
                    window.emit(
                        DRAG_DROP,
                        DropPayload {
                            selection_id: selection.selection_id,
                            position: pos(position),
                            names: names(paths),
                        },
                    )
                }
                DragDropEvent::Leave => window.emit(
                    DRAG_LEAVE,
                    DragPayload {
                        position: None,
                        names: Vec::new(),
                    },
                ),
                _ => Ok(()),
            };
            if let Err(e) = result {
                log::debug!("ドラッグのイベントを送れません: {e}");
            }
        }
        _ => {}
    }
}

/// 終了の要求。転送中なら確認し、終了する場合は 3 秒を上限に中止処理を行う（04 §14.4）。
pub fn request_quit(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let (active, _) = state.core.transfers().summary();
        if active > 0 {
            let dialog = app
                .dialog()
                .message(format!(
                    "{active} 件の転送が完了していません。終了すると転送を中止します。"
                ))
                .title("S3 Drive を終了しますか？")
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "終了".into(),
                    "キャンセル".into(),
                ));
            let confirmed = tauri::async_runtime::spawn_blocking(move || dialog.blocking_show())
                .await
                .unwrap_or(false);
            if !confirmed {
                return;
            }
        }
        state.quitting.store(true, Ordering::Relaxed);
        state
            .core
            .transfers()
            .shutdown(Duration::from_secs(3))
            .await;
        app.exit(0);
    });
}

pub fn on_run_event(app: &AppHandle, event: RunEvent) {
    match event {
        RunEvent::ExitRequested { api, code, .. } => {
            let quitting = app.state::<AppState>().quitting.load(Ordering::Relaxed);
            if code.is_none() && !quitting {
                api.prevent_exit();
                request_quit(app);
            }
        }
        // Dock アイコンのクリックでメインウィンドウを再表示する
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => show_main(app),
        _ => {}
    }
}

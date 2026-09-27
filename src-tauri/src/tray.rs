//! メニューバー常駐（03 §9.5）。

use std::sync::Mutex;

use tauri::image::Image;
use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Manager, Wry};

use crate::state::AppState;

const TRAY_ID: &str = "main";

/// 転送の状態を表示する項目（選択不可）。
pub struct TrayStatus(Mutex<(MenuItem<Wry>, String)>);

pub fn install(app: &App) -> tauri::Result<()> {
    let status = MenuItemBuilder::with_id("tray.status", "転送はありません")
        .enabled(false)
        .build(app)?;
    let menu = MenuBuilder::new(app)
        .item(&MenuItemBuilder::with_id("tray.open", "S3 Drive を開く").build(app)?)
        .separator()
        .item(&status)
        .separator()
        .item(&MenuItemBuilder::with_id("tray.upload", "アップロード…").build(app)?)
        .item(&MenuItemBuilder::with_id("tray.settings", "設定…").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("tray.quit", "S3 Drive を終了").build(app)?)
        .build()?;
    // DS: assets/menubar-glyph.svg から作ったテンプレート画像。OS が明暗に合わせて着色する
    let icon = Image::from_bytes(include_bytes!("../icons/tray-template.png"))?;
    let visible = app
        .state::<AppState>()
        .core
        .settings_store()
        .settings()
        .general
        .show_menu_bar_icon;
    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(true)
        .tooltip("S3 Drive")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .build(app)?;
    tray.set_visible(visible)?;
    app.manage(TrayStatus(Mutex::new((status, String::new()))));
    Ok(())
}

pub fn set_visible(app: &AppHandle, visible: bool) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_visible(visible);
    }
}

/// 10 進接頭辞のサイズ（02 §9.2）。
fn format_size(bytes: u64) -> String {
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let units = ["KB", "MB", "GB", "TB", "PB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    loop {
        value /= 1000.0;
        if value < 1000.0 || unit == units.len() - 1 {
            break;
        }
        unit += 1;
    }
    if value >= 100.0 {
        format!("{} {}", value.round(), units[unit])
    } else {
        format!("{:.1} {}", value, units[unit])
    }
}

/// 「転送中: {n} 件（残り {サイズ}）」または「転送はありません」。
pub fn status_text(active: usize, remaining: u64) -> String {
    if active == 0 {
        "転送はありません".to_string()
    } else {
        format!("転送中: {active} 件（残り {}）", format_size(remaining))
    }
}

pub fn refresh_status(app: &AppHandle) {
    let Some(status) = app.try_state::<TrayStatus>() else {
        return;
    };
    let (active, remaining) = app.state::<AppState>().core.transfers().summary();
    let text = status_text(active, remaining);
    let mut guard = status.0.lock().unwrap();
    if guard.1 != text {
        let _ = guard.0.set_text(&text);
        guard.1 = text;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_status() {
        assert_eq!(status_text(0, 0), "転送はありません");
        assert_eq!(status_text(3, 842_000_000), "転送中: 3 件（残り 842 MB）");
        assert_eq!(status_text(1, 4_200_000), "転送中: 1 件（残り 4.2 MB）");
        assert_eq!(format_size(999), "999 B");
        assert_eq!(format_size(248_600_000_000), "249 GB");
    }
}

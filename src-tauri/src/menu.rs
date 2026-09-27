//! アプリのメニューバー（03 §9.4）。
//!
//! 項目が選ばれたらフロントエンドへ `menu://action` で知らせる。有効・無効はフロントエンドが
//! `menu_update_state` で選択状態や表示中の画面を知らせて更新する。

use std::collections::HashMap;
use std::sync::Mutex;

use s3drive_core::model::{AppView, MenuAction, MenuState};
use tauri::menu::{
    AboutMetadataBuilder, CheckMenuItem, CheckMenuItemBuilder, Menu, MenuBuilder, MenuItem,
    MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder,
};
use tauri::{App, AppHandle, Emitter, Manager, Wry};
use tauri_plugin_opener::OpenerExt;

use crate::events::MENU_ACTION;
use crate::window::{MAIN, open_settings, request_quit, show_main};

const HELP_URL: &str = "https://github.com/Sh1gekicks/s3driveapp#readme";
const ISSUE_URL: &str = "https://github.com/Sh1gekicks/s3driveapp/issues/new";

#[derive(Default)]
pub struct MenuHandles {
    items: Mutex<HashMap<&'static str, MenuItem<Wry>>>,
    checks: Mutex<HashMap<&'static str, CheckMenuItem<Wry>>>,
}

fn item(
    app: &App,
    handles: &MenuHandles,
    id: &'static str,
    text: &str,
    accelerator: Option<&str>,
) -> tauri::Result<MenuItem<Wry>> {
    let mut b = MenuItemBuilder::with_id(id, text);
    if let Some(a) = accelerator {
        b = b.accelerator(a);
    }
    let i = b.build(app)?;
    handles.items.lock().unwrap().insert(id, i.clone());
    Ok(i)
}

fn check(
    app: &App,
    handles: &MenuHandles,
    id: &'static str,
    text: &str,
    accelerator: Option<&str>,
) -> tauri::Result<CheckMenuItem<Wry>> {
    let mut b = CheckMenuItemBuilder::with_id(id, text);
    if let Some(a) = accelerator {
        b = b.accelerator(a);
    }
    let i = b.build(app)?;
    handles.checks.lock().unwrap().insert(id, i.clone());
    Ok(i)
}

pub fn build(app: &App) -> tauri::Result<Menu<Wry>> {
    let h = MenuHandles::default();
    let about = AboutMetadataBuilder::new()
        .name(Some("S3 Drive"))
        .version(Some(app.package_info().version.to_string()))
        .build();
    let app_menu = SubmenuBuilder::new(app, "S3 Drive")
        .item(&PredefinedMenuItem::about(
            app,
            Some("S3 Drive について"),
            Some(about),
        )?)
        .item(&item(
            app,
            &h,
            "app.check_update",
            "アップデートを確認…",
            None,
        )?)
        .separator()
        .item(&item(
            app,
            &h,
            "app.settings",
            "設定…",
            Some("CmdOrCtrl+,"),
        )?)
        .separator()
        .item(&PredefinedMenuItem::services(app, Some("サービス"))?)
        .separator()
        .item(&PredefinedMenuItem::hide(app, Some("S3 Drive を隠す"))?)
        .item(&PredefinedMenuItem::hide_others(app, Some("ほかを隠す"))?)
        .item(&PredefinedMenuItem::show_all(app, Some("すべてを表示"))?)
        .separator()
        .item(&item(
            app,
            &h,
            "app.quit",
            "S3 Drive を終了",
            Some("CmdOrCtrl+Q"),
        )?)
        .build()?;

    let file = SubmenuBuilder::new(app, "ファイル")
        .item(&item(
            app,
            &h,
            "file.new_folder",
            "新規フォルダ",
            Some("CmdOrCtrl+Shift+N"),
        )?)
        .item(&item(
            app,
            &h,
            "file.upload",
            "アップロード…",
            Some("CmdOrCtrl+U"),
        )?)
        .item(&item(
            app,
            &h,
            "file.upload_folder",
            "フォルダをアップロード…",
            Some("CmdOrCtrl+Alt+U"),
        )?)
        .separator()
        .item(&item(
            app,
            &h,
            "file.download",
            "ダウンロード",
            Some("CmdOrCtrl+D"),
        )?)
        .item(&item(
            app,
            &h,
            "file.download_to",
            "場所を指定してダウンロード…",
            Some("CmdOrCtrl+Shift+D"),
        )?)
        .separator()
        .item(&item(app, &h, "file.rename", "名前を変更…", None)?)
        .item(&item(app, &h, "file.move", "移動…", None)?)
        .item(&item(
            app,
            &h,
            "file.storage_class",
            "ストレージクラスを変更…",
            None,
        )?)
        .item(&item(app, &h, "file.restore", "取り出し…", None)?)
        .separator()
        .item(&item(
            app,
            &h,
            "file.delete",
            "削除",
            Some("CmdOrCtrl+Backspace"),
        )?)
        .separator()
        .item(&PredefinedMenuItem::close_window(
            app,
            Some("ウインドウを閉じる"),
        )?)
        .build()?;

    // カット・コピー・ペーストは OS 定義の項目にする（編集メニューがないと入力欄で ⌘C／⌘V が効かない）
    let edit = SubmenuBuilder::new(app, "編集")
        .item(&PredefinedMenuItem::undo(app, Some("取り消す"))?)
        .item(&PredefinedMenuItem::redo(app, Some("やり直す"))?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, Some("カット"))?)
        .item(&PredefinedMenuItem::copy(app, Some("コピー"))?)
        .item(&PredefinedMenuItem::paste(app, Some("ペースト"))?)
        .item(&PredefinedMenuItem::select_all(app, Some("すべてを選択"))?)
        .separator()
        .item(&item(
            app,
            &h,
            "edit.copy_key",
            "キーをコピー",
            Some("CmdOrCtrl+Alt+C"),
        )?)
        .item(&item(app, &h, "edit.find", "検索", Some("CmdOrCtrl+F"))?)
        .build()?;

    // 表示切り替えのショートカットは Finder に合わせる（⌘1 がアイコン、⌘2 がリスト）
    let view = SubmenuBuilder::new(app, "表示")
        .item(&check(
            app,
            &h,
            "view.grid",
            "アイコン",
            Some("CmdOrCtrl+1"),
        )?)
        .item(&check(app, &h, "view.list", "リスト", Some("CmdOrCtrl+2"))?)
        .separator()
        .item(&check(
            app,
            &h,
            "view.inspector",
            "インスペクタを表示",
            Some("CmdOrCtrl+Alt+I"),
        )?)
        .item(&check(
            app,
            &h,
            "view.filters",
            "フィルタを表示",
            Some("CmdOrCtrl+Alt+F"),
        )?)
        .item(&check(
            app,
            &h,
            "view.hidden",
            "隠しファイルを表示",
            Some("CmdOrCtrl+Shift+Period"),
        )?)
        .item(&check(
            app,
            &h,
            "view.deleted",
            "削除済みの項目を表示",
            None,
        )?)
        .separator()
        .item(&item(
            app,
            &h,
            "view.reload",
            "再読み込み",
            Some("CmdOrCtrl+R"),
        )?)
        .separator()
        .item(&PredefinedMenuItem::fullscreen(
            app,
            Some("フルスクリーンにする"),
        )?)
        .build()?;

    let go = SubmenuBuilder::new(app, "移動")
        .item(&item(app, &h, "go.back", "戻る", Some("CmdOrCtrl+["))?)
        .item(&item(app, &h, "go.forward", "進む", Some("CmdOrCtrl+]"))?)
        .item(&item(app, &h, "go.up", "親フォルダ", Some("CmdOrCtrl+Up"))?)
        .item(&item(
            app,
            &h,
            "go.open",
            "選択項目を開く",
            Some("CmdOrCtrl+Down"),
        )?)
        .separator()
        .item(&item(app, &h, "go.dashboard", "ストレージとコスト", None)?)
        .build()?;

    let window = SubmenuBuilder::new(app, "ウインドウ")
        .item(&PredefinedMenuItem::minimize(app, Some("しまう"))?)
        .item(&PredefinedMenuItem::maximize(app, Some("拡大／縮小"))?)
        .separator()
        .item(&item(app, &h, "window.front", "すべてを手前に移動", None)?)
        .build()?;

    let help = SubmenuBuilder::new(app, "ヘルプ")
        .item(&item(app, &h, "help.help", "S3 Drive ヘルプ", None)?)
        .item(&item(app, &h, "help.logs", "ログフォルダを開く", None)?)
        .item(&item(app, &h, "help.report", "問題を報告…", None)?)
        .build()?;

    let menu = MenuBuilder::new(app)
        .items(&[&app_menu, &file, &edit, &view, &go, &window, &help])
        .build()?;
    app.manage(h);
    Ok(menu)
}

/// Rust 側で処理する項目以外は、フロントエンドに知らせる。
pub fn on_menu_event(app: &AppHandle, id: &str) {
    match id {
        "app.settings" => {
            if let Err(e) = open_settings(app) {
                log::warn!("設定ウィンドウを開けません: {e}");
            }
        }
        "app.quit" | "tray.quit" => request_quit(app),
        "window.front" | "tray.open" => show_main(app),
        "help.help" => {
            let _ = app.opener().open_url(HELP_URL, None::<&str>);
        }
        "help.report" => {
            let _ = app.opener().open_url(ISSUE_URL, None::<&str>);
        }
        "help.logs" => {
            if let Err(e) = crate::commands::app::app_open_logs(app.clone()) {
                log::warn!("ログフォルダを開けません: {}", e.message);
            }
        }
        "tray.settings" => {
            let _ = open_settings(app);
        }
        other => {
            // メニューバー常駐の「アップロード…」はメインウィンドウを表示してから行う
            if other.starts_with("tray.") || other == "go.dashboard" {
                show_main(app);
            }
            if let Err(e) = app.emit_to(
                MAIN,
                MENU_ACTION,
                MenuAction {
                    id: other.to_string(),
                },
            ) {
                log::debug!("メニューのイベントを送れません: {e}");
            }
        }
    }
}

/// 選択状態・画面に合わせて項目の有効・無効とチェックを更新する。
pub fn update(app: &AppHandle, s: &MenuState) {
    let Some(h) = app.try_state::<MenuHandles>() else {
        return;
    };
    let files = s.view == AppView::Files;
    let any = files && s.selection_count > 0;
    let enable: &[(&str, bool)] = &[
        ("file.new_folder", files),
        ("file.upload", files),
        ("file.upload_folder", files),
        ("file.download", any),
        ("file.download_to", any),
        ("file.rename", files && s.selection_count == 1),
        ("file.move", any),
        ("file.storage_class", any),
        ("file.restore", any && s.has_archived),
        ("file.delete", any),
        ("edit.copy_key", any),
        ("edit.find", files),
        ("view.reload", s.view != AppView::Signin),
        ("go.back", s.can_go_back),
        ("go.forward", s.can_go_forward),
        ("go.up", files && s.can_go_up),
        ("go.open", any),
        ("go.dashboard", s.view != AppView::Signin),
    ];
    let items = h.items.lock().unwrap();
    for (id, on) in enable {
        if let Some(i) = items.get(id) {
            let _ = i.set_enabled(*on);
        }
    }
    let checks = h.checks.lock().unwrap();
    let checked: &[(&str, bool, bool)] = &[
        ("view.grid", s.view_mode == "grid", files),
        ("view.list", s.view_mode == "list", files),
        ("view.inspector", s.inspector_visible, files),
        ("view.filters", s.filters_visible, files),
        ("view.hidden", s.show_hidden, true),
        ("view.deleted", s.show_deleted, files),
    ];
    for (id, value, on) in checked {
        if let Some(c) = checks.get(id) {
            let _ = c.set_checked(*value);
            let _ = c.set_enabled(*on);
        }
    }
}

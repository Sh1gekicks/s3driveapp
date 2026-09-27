//! S3 Drive（Tauri v2 アプリ本体）。IPC コマンド、プラグインの登録、ウィンドウ・メニュー・トレイ（05 §1.1）。

mod background;
mod commands;
mod events;
mod menu;
mod state;
mod tray;
mod window;

use tauri::generate_handler;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

/// ログ: 10 MB で切り替え、5 世代（01 §7.3）。レベルは設定を読んでから `log::set_max_level` で絞る。
fn log_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let mut targets = vec![Target::new(TargetKind::LogDir {
        file_name: Some("s3drive".into()),
    })];
    if cfg!(debug_assertions) {
        targets.push(Target::new(TargetKind::Stdout));
        targets.push(Target::new(TargetKind::Webview));
    }
    tauri_plugin_log::Builder::new()
        .clear_targets()
        .targets(targets)
        .level(log::LevelFilter::Debug)
        // 依存ライブラリの詳細なログは出さない（オブジェクトのキーなどを含みうるため）
        .level_for("aws_smithy_runtime", log::LevelFilter::Warn)
        .level_for("aws_config", log::LevelFilter::Warn)
        .level_for("hyper", log::LevelFilter::Warn)
        .level_for("rustls", log::LevelFilter::Warn)
        .max_file_size(10_000_000)
        .rotation_strategy(RotationStrategy::KeepSome(5))
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        // 二重起動を防ぎ、2 つ目の起動は既存ウィンドウを前面に出す
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            window::show_main(app)
        }))
        .plugin(log_plugin())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_denylist(&[window::SETTINGS])
                .build(),
        );
    #[cfg(feature = "e2e")]
    let builder = builder.plugin(tauri_plugin_wdio_webdriver::init());

    builder
        .setup(|app| {
            state::init(app)?;
            let menu = menu::build(app)?;
            app.set_menu(menu)?;
            app.on_menu_event(|app, event| menu::on_menu_event(app, event.id().as_ref()));
            tray::install(app)?;
            window::setup_main(app)?;
            background::start(app.handle());
            Ok(())
        })
        .on_window_event(window::on_window_event)
        .invoke_handler(app_commands!(generate_handler))
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(window::on_run_event);
}

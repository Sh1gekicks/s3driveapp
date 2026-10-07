//! 背景の処理（メニューバー常駐の状態表示、取り出しの確認、自動更新の確認、インデックスの更新通知）。

use std::time::Duration;

use s3drive_core::model::{IndexUpdated, UpdateInfo};
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

use crate::events::{INDEX_UPDATED, RESTORE_COMPLETED, UPDATE_AVAILABLE, emit_all};
use crate::state::AppState;

/// 取り出しの状態を確認する間隔（04 §8.4）。
const RESTORE_CHECK_INTERVAL: Duration = Duration::from_secs(15 * 60);
/// 更新を確認する間隔（08 §7）。
const UPDATE_CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

pub fn start(app: &AppHandle) {
    let state = app.state::<AppState>();
    let handle = app.clone();
    state
        .core
        .set_index_listener(Box::new(move |connection_id, status| {
            emit_all(
                &handle,
                INDEX_UPDATED,
                IndexUpdated {
                    connection_id: connection_id.to_string(),
                    status: status.clone(),
                },
            );
        }));

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            crate::tray::refresh_status(&handle);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(60)).await;
        loop {
            check_restores(&handle).await;
            tokio::time::sleep(RESTORE_CHECK_INTERVAL).await;
        }
    });

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(10)).await;
        loop {
            let auto = handle
                .state::<AppState>()
                .core
                .settings_store()
                .settings()
                .advanced
                .auto_check_update;
            if auto && !cfg!(debug_assertions) {
                check_update(&handle).await;
            }
            tokio::time::sleep(UPDATE_CHECK_INTERVAL).await;
        }
    });
}

/// 完了したものは `restore://completed` で知らせる。トーストと macOS の通知（設定「完了時に通知する」に従う）は
/// メインウィンドウのフロントエンドが出す（03 §11）。ここでも通知すると二重になり、設定でも止められない。
async fn check_restores(app: &AppHandle) {
    let core = app.state::<AppState>().core.clone();
    match core.objects().check_restores().await {
        Ok(done) => {
            for c in done {
                emit_all(app, RESTORE_COMPLETED, c);
            }
        }
        Err(e) => log::debug!("取り出しの状態を確認できません: {e}"),
    }
}

async fn check_update(app: &AppHandle) {
    // インストール済みで、転送の完了後の再起動を待っている（08 §7）
    if app
        .state::<AppState>()
        .restart_pending
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        return;
    }
    let Ok(updater) = app.updater() else { return };
    match updater.check().await {
        Ok(Some(update)) => emit_all(
            app,
            UPDATE_AVAILABLE,
            UpdateInfo {
                version: update.version.clone(),
                notes: update.body.clone(),
            },
        ),
        Ok(None) => {}
        Err(e) => log::info!("アップデートを確認できません: {e}"),
    }
}

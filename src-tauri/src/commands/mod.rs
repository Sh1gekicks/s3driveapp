//! IPC コマンド（05 §3）。引数の検証とエラー変換以外の処理は書かない（05 §6.3）。

pub mod app;
pub mod auth;
pub mod connections;
pub mod metrics;
pub mod objects;
pub mod search;
pub mod transfers;
pub mod versions;

use s3drive_core::AppError;

pub type CmdResult<T> = Result<T, AppError>;

/// `Result<T, CoreError>` を IPC の形式に変換する。
pub fn ipc<T>(r: s3drive_core::CoreResult<T>) -> CmdResult<T> {
    r.map_err(AppError::from)
}

/// コマンドの一覧。`build.rs` の `AppManifest` と、`generate_handler!` の両方に使う。
#[macro_export]
macro_rules! app_commands {
    ($m:ident) => {
        $m![
            $crate::commands::auth::auth_get_session,
            $crate::commands::auth::auth_restore,
            $crate::commands::auth::auth_sign_in,
            $crate::commands::auth::auth_cancel_sign_in,
            $crate::commands::auth::auth_sign_out,
            $crate::commands::connections::connection_list,
            $crate::commands::connections::connection_test,
            $crate::commands::connections::connection_create,
            $crate::commands::connections::connection_update,
            $crate::commands::connections::connection_patch,
            $crate::commands::connections::connection_delete,
            $crate::commands::connections::connection_reorder,
            $crate::commands::connections::connection_last_location,
            $crate::commands::connections::connection_set_location,
            $crate::commands::connections::credential_list,
            $crate::commands::connections::credential_update,
            $crate::commands::connections::bucket_get_info,
            $crate::commands::objects::objects_list_page,
            $crate::commands::objects::object_head,
            $crate::commands::objects::folder_create,
            $crate::commands::objects::folder_summary,
            $crate::commands::objects::folder_children,
            $crate::commands::objects::objects_find_conflicts,
            $crate::commands::objects::objects_delete,
            $crate::commands::objects::objects_move,
            $crate::commands::objects::object_rename,
            $crate::commands::objects::objects_change_storage_class,
            $crate::commands::objects::objects_request_restore,
            $crate::commands::versions::versions_list,
            $crate::commands::versions::version_restore,
            $crate::commands::versions::version_delete,
            $crate::commands::versions::deleted_restore,
            $crate::commands::transfers::transfer_subscribe,
            $crate::commands::transfers::pick_upload_files,
            $crate::commands::transfers::upload_prepare,
            $crate::commands::transfers::upload_start,
            $crate::commands::transfers::pick_download_dir,
            $crate::commands::transfers::download_start,
            $crate::commands::transfers::download_reveal,
            $crate::commands::transfers::job_cancel,
            $crate::commands::transfers::transfer_retry,
            $crate::commands::transfers::transfer_clear_finished,
            $crate::commands::search::search_query,
            $crate::commands::search::search_index_status,
            $crate::commands::search::search_index_rebuild,
            $crate::commands::search::search_index_delete,
            $crate::commands::metrics::metrics_storage,
            $crate::commands::metrics::cost_summary,
            $crate::commands::metrics::cost_refresh,
            $crate::commands::metrics::pricing_get,
            $crate::commands::app::settings_get,
            $crate::commands::app::settings_update,
            $crate::commands::app::app_set_theme,
            $crate::commands::app::app_open_settings,
            $crate::commands::app::app_startup_info,
            $crate::commands::app::menu_update_state,
            $crate::commands::app::app_open_logs,
            $crate::commands::app::app_clear_cache,
            $crate::commands::app::app_check_update,
            $crate::commands::app::app_install_update,
            $crate::commands::app::app_notify,
            $crate::commands::app::app_choose_download_dir,
        ]
    };
}

// リリースビルドで Windows のコンソールを出さない（移植の余地のため残す）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    s3drive_app_lib::run();
}

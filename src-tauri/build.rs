//! アプリのコマンドを `AppManifest` に列挙し、ウィンドウごとの capability で許可するコマンドを明示する（05 §8）。
//!
//! コマンドの一覧は `src/commands/mod.rs` の `app_commands!` を正とし、ここで読み取る。

fn main() {
    let source = std::fs::read_to_string("src/commands/mod.rs").expect("read src/commands/mod.rs");
    let commands: Vec<&'static str> = source
        .lines()
        .filter_map(|l| l.trim().strip_prefix("$crate::commands::"))
        .filter_map(|l| l.trim_end_matches(',').rsplit("::").next())
        .map(|name| &*Box::leak(name.to_string().into_boxed_str()))
        .collect();
    assert!(!commands.is_empty(), "no commands found");
    // AppManifest は 'static のスライスを受け取る（ビルドスクリプトなので解放しなくてよい）
    let commands: &'static [&'static str] = Box::leak(commands.into_boxed_slice());
    println!("cargo:rerun-if-changed=src/commands/mod.rs");
    for var in [
        "S3DRIVE_GOOGLE_CLIENT_ID",
        "S3DRIVE_GOOGLE_CLIENT_SECRET",
        "S3DRIVE_ALLOWED_EMAILS",
        "S3DRIVE_ALLOWED_DOMAINS",
    ] {
        println!("cargo:rerun-if-env-changed={var}");
    }
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(commands)),
    )
    .expect("failed to run tauri-build");
}

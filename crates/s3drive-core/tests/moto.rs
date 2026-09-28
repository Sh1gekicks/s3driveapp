//! moto（S3 のモック）を使う結合テスト（09 §2.2）。
//!
//! `S3DRIVE_TEST_ENDPOINT`（例: `http://localhost:5000`）が設定されていない場合は何もしない。
//!   docker compose -f docker-compose.test.yml up -d
//!   S3DRIVE_TEST_ENDPOINT=http://localhost:5000 cargo test -p s3drive-core --test moto

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{
    BucketLocationConstraint, BucketVersioningStatus, CreateBucketConfiguration,
    VersioningConfiguration,
};
use s3drive_core::jobs::MemorySink;
use s3drive_core::model::*;
use s3drive_core::store::db::tempfile_guard::TempDir;
use s3drive_core::{Core, ErrorCode};

const REGION: &str = "ap-northeast-1";
// moto はどの認証情報でも受け付ける。シークレットスキャンに実在のキーと誤検知されないよう、
// アクセスキー ID は分けて書き、シークレットも明らかにテスト用とわかる値にする
const ACCESS_KEY: &str = concat!("AKIA", "TESTFAKEKEY00000");
const SECRET_KEY: &str = "test-secret-access-key-for-s3drive-00000";

fn endpoint() -> Option<String> {
    std::env::var("S3DRIVE_TEST_ENDPOINT")
        .ok()
        .filter(|s| !s.is_empty())
}

macro_rules! require_moto {
    () => {
        match endpoint() {
            Some(e) => e,
            None => {
                eprintln!("S3DRIVE_TEST_ENDPOINT が未設定のためスキップします");
                return;
            }
        }
    };
}

struct Env {
    core: Core,
    _dir: TempDir,
    conn: String,
    bucket: String,
    s3: aws_sdk_s3::Client,
}

async fn raw_client(endpoint: &str) -> aws_sdk_s3::Client {
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(aws_config::Region::new(REGION))
        .credentials_provider(aws_credential_types::Credentials::new(
            ACCESS_KEY, SECRET_KEY, None, None, "test",
        ))
        .endpoint_url(endpoint)
        .load()
        .await;
    aws_sdk_s3::Client::from_conf(
        aws_sdk_s3::config::Builder::from(&config)
            .force_path_style(true)
            .request_checksum_calculation(
                aws_sdk_s3::config::RequestChecksumCalculation::WhenRequired,
            )
            .build(),
    )
}

async fn setup(endpoint: &str, versioning: bool) -> Env {
    let s3 = raw_client(endpoint).await;
    let bucket = format!(
        "s3drive-test-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..12]
    );
    s3.create_bucket()
        .bucket(&bucket)
        .create_bucket_configuration(
            CreateBucketConfiguration::builder()
                .location_constraint(BucketLocationConstraint::from(REGION))
                .build(),
        )
        .send()
        .await
        .unwrap();
    if versioning {
        s3.put_bucket_versioning()
            .bucket(&bucket)
            .versioning_configuration(
                VersioningConfiguration::builder()
                    .status(BucketVersioningStatus::Enabled)
                    .build(),
            )
            .send()
            .await
            .unwrap();
    }
    let (core, dir) = Core::for_tests(Some(endpoint.to_string())).unwrap();
    core.auth_sign_in(&|_| Ok(())).await.unwrap();
    let conn = core
        .connections()
        .create(ConnectionInput {
            bucket: bucket.clone(),
            region: REGION.into(),
            credential: CredentialInput::New {
                access_key_id: ACCESS_KEY.into(),
                secret_access_key: SECRET_KEY.into(),
            },
            role_arn: None,
            external_id: None,
        })
        .await
        .unwrap()
        .id;
    Env {
        core,
        _dir: dir,
        conn,
        bucket,
        s3,
    }
}

impl Env {
    async fn put(&self, key: &str, body: &[u8]) {
        self.s3
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(body.to_vec()))
            .send()
            .await
            .unwrap();
    }

    async fn list(&self, prefix: &str, opts: ListOptions) -> Vec<Entry> {
        let mut token = None;
        let mut all = Vec::new();
        loop {
            let page = self
                .core
                .objects()
                .list_page(&self.conn, prefix, token, opts)
                .await
                .unwrap();
            all.extend(page.entries);
            token = page.next_token;
            if token.is_none() {
                return all;
            }
        }
    }

    async fn keys(&self, prefix: &str) -> Vec<String> {
        let mut keys: Vec<String> = self
            .list(
                prefix,
                ListOptions {
                    show_hidden: true,
                    include_deleted: false,
                },
            )
            .await
            .iter()
            .map(|e| e.key().to_string())
            .collect();
        keys.sort();
        keys
    }

    async fn exists(&self, key: &str) -> bool {
        self.s3
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .is_ok()
    }
}

async fn wait_batch(sink: &MemorySink<BatchEvent>) -> BatchResult {
    for _ in 0..600 {
        if let Some(BatchEvent::Finished(f)) = sink
            .events()
            .into_iter()
            .find(|e| matches!(e, BatchEvent::Finished(_)))
        {
            return f.result;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("batch job did not finish");
}

async fn wait_transfer(core: &Core, job_id: &str) -> TransferJob {
    for _ in 0..1200 {
        if let Some(job) = core
            .transfers()
            .jobs()
            .into_iter()
            .find(|j| j.job_id == job_id)
            && matches!(
                job.status,
                JobStatus::Succeeded | JobStatus::Failed | JobStatus::Canceled
            )
        {
            return job;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("transfer did not finish");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn connection_test_reports_versioning_and_errors() {
    let endpoint = require_moto!();
    let env = setup(&endpoint, true).await;
    let result = env
        .core
        .connections()
        .test(&ConnectionInput {
            bucket: env.bucket.clone(),
            region: REGION.into(),
            credential: CredentialInput::New {
                access_key_id: ACCESS_KEY.into(),
                secret_access_key: SECRET_KEY.into(),
            },
            role_arn: None,
            external_id: None,
        })
        .await
        .unwrap();
    assert_eq!(result.versioning, Versioning::Enabled);
    assert_eq!(result.region, REGION);

    let missing = env
        .core
        .connections()
        .test(&ConnectionInput {
            bucket: "no-such-bucket-s3drive".into(),
            region: REGION.into(),
            credential: CredentialInput::New {
                access_key_id: ACCESS_KEY.into(),
                secret_access_key: SECRET_KEY.into(),
            },
            role_arn: None,
            external_id: None,
        })
        .await
        .unwrap_err();
    assert_eq!(missing.code, ErrorCode::BucketNotFound);
    assert_eq!(env.core.connections().list().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn deletes_connections_and_unused_credentials() {
    let endpoint = require_moto!();
    let env = setup(&endpoint, false).await;
    let connections = env.core.connections();
    let credential_id = connections.list().unwrap()[0].credential_id.clone();
    let second = connections
        .create(ConnectionInput {
            bucket: env.bucket.clone(),
            region: REGION.into(),
            credential: CredentialInput::Existing {
                credential_id: credential_id.clone(),
            },
            role_arn: None,
            external_id: None,
        })
        .await
        .unwrap();
    connections
        .set_last_location(Location {
            connection_id: env.conn.clone(),
            prefix: "a/".into(),
        })
        .unwrap();

    connections.delete(&env.conn).await.unwrap();
    let ids: Vec<_> = connections
        .list()
        .unwrap()
        .into_iter()
        .map(|c| c.id)
        .collect();
    assert_eq!(ids, vec![second.id.clone()]);
    assert!(connections.last_location().unwrap().is_none());
    // 残りの接続が使っている認証情報は消さない
    assert_eq!(connections.credential_list().unwrap().len(), 1);

    connections.delete(&second.id).await.unwrap();
    assert!(connections.list().unwrap().is_empty());
    assert!(connections.credential_list().unwrap().is_empty());
    assert_eq!(
        connections.delete(&second.id).await.unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lists_folders_pages_and_special_keys() {
    let endpoint = require_moto!();
    let env = setup(&endpoint, false).await;
    env.put("docs/", b"").await;
    env.put("docs/a b.txt", b"1").await;
    env.put("docs/日本語 ファイル.pdf", b"22").await;
    env.put("docs/x+y.txt", b"333").await;
    env.put("docs/sub/deep.txt", b"4444").await;
    env.put("docs/.hidden", b"5").await;

    let entries = env.list("docs/", ListOptions::default()).await;
    let names: Vec<&str> = entries.iter().map(Entry::name).collect();
    assert!(names.contains(&"sub"));
    assert!(names.contains(&"a b.txt"));
    assert!(names.contains(&"日本語 ファイル.pdf"));
    assert!(names.contains(&"x+y.txt"));
    // フォルダマーカーと隠しファイルは表示しない
    assert!(!names.contains(&""));
    assert!(!names.contains(&".hidden"));
    let with_hidden = env
        .list(
            "docs/",
            ListOptions {
                show_hidden: true,
                include_deleted: false,
            },
        )
        .await;
    assert_eq!(with_hidden.len(), entries.len() + 1);

    // 2,500 件のページング
    for i in 0..2500 {
        env.put(&format!("many/{i:05}.txt"), b"x").await;
    }
    let first = env
        .core
        .objects()
        .list_page(&env.conn, "many/", None, ListOptions::default())
        .await
        .unwrap();
    assert_eq!(first.entries.len(), 1000);
    assert!(first.next_token.is_some());
    assert_eq!(env.list("many/", ListOptions::default()).await.len(), 2500);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn uploads_single_and_multipart_with_conflicts() {
    let endpoint = require_moto!();
    let env = setup(&endpoint, true).await;
    env.core
        .settings_store()
        .patch(serde_json::json!({ "transfer": { "multipartThresholdMb": 8 } }))
        .unwrap();
    let dir = TempDir::new().unwrap();
    let small = dir.path().join("small.txt");
    std::fs::write(&small, b"hello").unwrap();
    let big = dir.path().join("big.bin");
    let big_data: Vec<u8> = (0..(20 * 1024 * 1024u32))
        .map(|i| (i % 251) as u8)
        .collect();
    std::fs::write(&big, &big_data).unwrap();
    let folder = dir.path().join("folder");
    std::fs::create_dir_all(folder.join("empty")).unwrap();
    // NFD（濁点が分解された形）のファイル名
    std::fs::write(folder.join("\u{30CF}\u{309A}\u{30F3}.txt"), b"bread").unwrap();
    std::fs::write(folder.join(".DS_Store"), b"junk").unwrap();

    env.put("up/small.txt", b"old").await;

    let selection =
        env.core
            .selections()
            .register(vec![small.clone(), big.clone(), folder.clone()]);
    let plan = env
        .core
        .upload_prepare(&env.conn, "up/", &selection.selection_id)
        .await
        .unwrap();
    assert_eq!(plan.file_count, 3);
    assert_eq!(plan.conflicts.len(), 1);
    assert_eq!(plan.conflicts[0].key, "up/small.txt");
    assert!(plan.versioning_enabled);
    assert!(
        plan.excluded
            .iter()
            .any(|e| e.name == ".DS_Store" && e.reason == ExcludeReason::Ignored)
    );

    let job = env
        .core
        .upload_start(
            &plan.plan_id,
            Decisions::All {
                all: ConflictDecision::KeepBoth,
            },
        )
        .await
        .unwrap();
    let job = wait_transfer(&env.core, &job).await;
    assert_eq!(job.status, JobStatus::Succeeded, "{job:?}");
    assert_eq!(job.done_bytes, job.total_bytes);

    let keys = env.keys("up/").await;
    assert!(keys.contains(&"up/small (1).txt".to_string()), "{keys:?}");
    assert!(keys.contains(&"up/big.bin".to_string()));
    assert!(keys.contains(&"up/folder/".to_string()));
    assert!(env.exists("up/folder/パン.txt").await);
    assert!(env.exists("up/folder/empty/").await);
    assert!(!env.exists("up/folder/.DS_Store").await);

    let detail = env
        .core
        .objects()
        .head(&env.conn, "up/big.bin", None)
        .await
        .unwrap();
    assert_eq!(detail.size, big_data.len() as u64);
    assert!(detail.user_metadata.contains_key("s3drive-mtime"));
    assert_eq!(detail.content_type, "application/octet-stream");
    // マルチパートで送った内容が一致する
    let got = env
        .s3
        .get_object()
        .bucket(&env.bucket)
        .key("up/big.bin")
        .send()
        .await
        .unwrap();
    assert_eq!(
        got.body.collect().await.unwrap().into_bytes().as_ref(),
        &big_data[..]
    );
    // 作成日はバージョン一覧から決まる
    assert_eq!(detail.created.source, CreatedSource::OldestVersion);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn downloads_files_folders_and_ranges() {
    let endpoint = require_moto!();
    let env = setup(&endpoint, false).await;
    let big: Vec<u8> = (0..(70 * 1_000_000u32)).map(|i| (i % 253) as u8).collect();
    env.s3
        .put_object()
        .bucket(&env.bucket)
        .key("data/big.bin")
        .metadata("s3drive-mtime", "2025-01-02T03:04:05Z")
        .body(ByteStream::from(big.clone()))
        .send()
        .await
        .unwrap();
    env.put("data/sub/a.txt", b"aaa").await;
    env.put("data/sub/empty/", b"").await;
    env.put("single.txt", b"single").await;

    let out = TempDir::new().unwrap();
    std::fs::write(out.path().join("single.txt"), b"existing").unwrap();
    let sel = env
        .core
        .selections()
        .register(vec![out.path().to_path_buf()]);
    let dest = DownloadDestination::Selection {
        selection_id: sel.selection_id,
    };
    let job = env
        .core
        .download_start(
            &env.conn,
            vec![Target::file("single.txt"), Target::folder("data/")],
            dest,
        )
        .await
        .unwrap();
    let job = wait_transfer(&env.core, &job).await;
    assert_eq!(job.status, JobStatus::Succeeded, "{job:?}");

    // 同名のファイルがあれば連番を付ける
    assert_eq!(
        std::fs::read(out.path().join("single (1).txt")).unwrap(),
        b"single"
    );
    assert_eq!(
        std::fs::read(out.path().join("single.txt")).unwrap(),
        b"existing"
    );
    let downloaded = std::fs::read(out.path().join("data/big.bin")).unwrap();
    assert_eq!(downloaded.len(), big.len());
    assert!(downloaded == big);
    assert!(out.path().join("data/sub/empty").is_dir());
    assert_eq!(
        std::fs::read(out.path().join("data/sub/a.txt")).unwrap(),
        b"aaa"
    );
    // 更新日時を復元する
    let mtime = std::fs::metadata(out.path().join("data/big.bin"))
        .unwrap()
        .modified()
        .unwrap();
    let secs = mtime
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert_eq!(secs, 1_735_787_045);
    assert!(
        std::fs::read_dir(out.path().join("data"))
            .unwrap()
            .all(|e| !e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".s3drive-download"))
    );
    let saved: Vec<PathBuf> = env.core.transfers().saved_paths(&job.job_id);
    assert_eq!(saved.len(), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn deletes_with_markers_all_versions_and_large_folders() {
    let endpoint = require_moto!();
    let env = setup(&endpoint, true).await;
    env.put("a.txt", b"1").await;
    env.put("a.txt", b"2").await;
    env.put("b.txt", b"1").await;
    for i in 0..1500 {
        env.put(&format!("bulk/{i:04}.txt"), b"x").await;
    }

    // 削除マーカー
    let sink = Arc::new(MemorySink::default());
    env.core
        .objects()
        .delete(&env.conn, vec![Target::file("a.txt")], false, sink.clone())
        .await
        .unwrap();
    assert_eq!(wait_batch(&sink).await.succeeded, 1);
    let deleted = env
        .list(
            "",
            ListOptions {
                show_hidden: false,
                include_deleted: true,
            },
        )
        .await;
    let a = deleted
        .iter()
        .find(|e| e.key() == "a.txt")
        .expect("deleted item is listed");
    assert!(matches!(a, Entry::File { deleted: true, .. }));

    // 削除済みの項目の復元
    let restored = env
        .core
        .versions()
        .undelete(&env.conn, &["a.txt".into()])
        .await
        .unwrap();
    assert_eq!(restored.succeeded, 1);
    assert!(env.exists("a.txt").await);

    // 全バージョンを完全に削除
    let sink = Arc::new(MemorySink::default());
    env.core
        .objects()
        .delete(&env.conn, vec![Target::file("a.txt")], true, sink.clone())
        .await
        .unwrap();
    // 2 つのバージョンを削除しても、件数は項目（キー）で数える（DLG-02）
    assert_eq!(wait_batch(&sink).await.succeeded, 1);
    let versions = env
        .core
        .versions()
        .list(&env.conn, "a.txt", None)
        .await
        .unwrap();
    assert!(versions.versions.is_empty());

    // 1,500 件のフォルダ（DeleteObjects 2 回に分かれる）
    let sink = Arc::new(MemorySink::default());
    env.core
        .objects()
        .delete(
            &env.conn,
            vec![Target::folder("bulk/")],
            false,
            sink.clone(),
        )
        .await
        .unwrap();
    let r = wait_batch(&sink).await;
    assert_eq!(r.succeeded, 1500);
    assert!(r.failed.is_empty());
    assert!(env.keys("bulk/").await.is_empty());
    assert!(env.exists("b.txt").await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn creates_moves_and_renames_folders() {
    let endpoint = require_moto!();
    let env = setup(&endpoint, false).await;
    let created = env
        .core
        .objects()
        .create_folder(&env.conn, "", "新規フォルダ")
        .await
        .unwrap();
    assert_eq!(created.key(), "新規フォルダ/");
    let dup = env
        .core
        .objects()
        .create_folder(&env.conn, "", "新規フォルダ")
        .await
        .unwrap_err();
    assert_eq!(dup.code, ErrorCode::AlreadyExists);
    assert_eq!(
        env.core
            .objects()
            .create_folder(&env.conn, "", "a/b")
            .await
            .unwrap_err()
            .code,
        ErrorCode::InvalidName
    );

    env.s3
        .put_object()
        .bucket(&env.bucket)
        .key("src/doc.txt")
        .storage_class(aws_sdk_s3::types::StorageClass::StandardIa)
        .metadata("owner", "yuki")
        .body(ByteStream::from_static(b"content"))
        .send()
        .await
        .unwrap();
    env.put("src/inner/x.txt", b"x").await;
    env.put("dest/", b"").await;

    let conflicts = env
        .core
        .objects()
        .find_conflicts(&env.conn, &[Target::folder("src/")], "dest/")
        .await
        .unwrap();
    assert!(conflicts.is_empty());

    let sink = Arc::new(MemorySink::default());
    env.core
        .objects()
        .move_objects(
            &env.conn,
            vec![Target::folder("src/")],
            "dest/".into(),
            Decisions::default(),
            sink.clone(),
        )
        .await
        .unwrap();
    let r = wait_batch(&sink).await;
    assert_eq!(r.succeeded, 2, "{r:?}");
    assert!(!env.exists("src/doc.txt").await);
    let moved = env
        .core
        .objects()
        .head(&env.conn, "dest/src/doc.txt", None)
        .await
        .unwrap();
    // クラスとメタデータを引き継ぐ
    assert_eq!(moved.storage_class, StorageClass::StandardIa);
    assert_eq!(
        moved.user_metadata.get("owner").map(String::as_str),
        Some("yuki")
    );

    // 名前の変更
    let sink = Arc::new(MemorySink::default());
    env.core
        .objects()
        .rename(
            &env.conn,
            Target::folder("dest/src/"),
            "renamed",
            sink.clone(),
        )
        .await
        .unwrap();
    assert_eq!(wait_batch(&sink).await.succeeded, 2);
    assert!(env.exists("dest/renamed/inner/x.txt").await);

    let summary = env
        .core
        .objects()
        .folder_summary(&env.conn, "dest/")
        .await
        .unwrap();
    assert_eq!(summary.item_count, 1);
    assert_eq!(summary.total_bytes, 8);
    let children = env
        .core
        .objects()
        .folder_children(&env.conn, "dest/")
        .await
        .unwrap();
    assert_eq!(children.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn changes_storage_class_and_skips_same_class() {
    let endpoint = require_moto!();
    let env = setup(&endpoint, false).await;
    env.put("c/one.txt", b"1").await;
    env.put("c/two.txt", b"2").await;
    let sink = Arc::new(MemorySink::default());
    env.core
        .objects()
        .change_storage_class(
            &env.conn,
            vec![Target::folder("c/")],
            StorageClass::GlacierIr,
            sink.clone(),
        )
        .await
        .unwrap();
    assert_eq!(wait_batch(&sink).await.succeeded, 2);
    let d = env
        .core
        .objects()
        .head(&env.conn, "c/one.txt", None)
        .await
        .unwrap();
    assert_eq!(d.storage_class, StorageClass::GlacierIr);

    let sink = Arc::new(MemorySink::default());
    env.core
        .objects()
        .change_storage_class(
            &env.conn,
            vec![Target::file("c/one.txt")],
            StorageClass::GlacierIr,
            sink.clone(),
        )
        .await
        .unwrap();
    let r = wait_batch(&sink).await;
    assert_eq!((r.succeeded, r.skipped.len()), (0, 1));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn manages_versions() {
    let endpoint = require_moto!();
    let env = setup(&endpoint, true).await;
    env.put("v.txt", b"first").await;
    tokio::time::sleep(Duration::from_millis(1100)).await;
    env.put("v.txt", b"second!").await;
    env.put("v.txt.bak", b"other key").await;

    let page = env
        .core
        .versions()
        .list(&env.conn, "v.txt", None)
        .await
        .unwrap();
    assert_eq!(page.versions.len(), 2, "exact key only");
    assert!(page.versions[0].is_latest);
    let oldest = page.versions[1].clone();
    assert_eq!(oldest.size, Some(5));

    let restored = env
        .core
        .versions()
        .restore(&env.conn, "v.txt", &oldest.version_id)
        .await
        .unwrap();
    assert!(restored.is_latest);
    let got = env
        .s3
        .get_object()
        .bucket(&env.bucket)
        .key("v.txt")
        .send()
        .await
        .unwrap();
    assert_eq!(
        got.body.collect().await.unwrap().into_bytes().as_ref(),
        b"first"
    );
    assert_eq!(
        env.core
            .versions()
            .list(&env.conn, "v.txt", None)
            .await
            .unwrap()
            .versions
            .len(),
        3
    );

    env.core
        .versions()
        .delete(&env.conn, "v.txt", &oldest.version_id)
        .await
        .unwrap();
    assert_eq!(
        env.core
            .versions()
            .list(&env.conn, "v.txt", None)
            .await
            .unwrap()
            .versions
            .len(),
        2
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn builds_and_queries_the_search_index() {
    let endpoint = require_moto!();
    let env = setup(&endpoint, false).await;
    env.put("projects/2026/report-q3.pdf", b"pdf").await;
    env.put("projects/2026/sales.csv", b"csv!").await;
    env.put("reports/", b"").await;
    env.put("root.txt", b"r").await;

    let sink = Arc::new(MemorySink::default());
    env.core
        .search()
        .rebuild(&env.conn, sink.clone())
        .await
        .unwrap();
    for _ in 0..200 {
        if sink
            .events()
            .iter()
            .any(|e| matches!(e, IndexEvent::Finished { .. }))
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let status = env.core.search().status(&env.conn).await.unwrap();
    assert_eq!(status.state, IndexState::Ready);
    assert_eq!(status.object_count, 3);

    let result = env
        .core
        .search()
        .query(&env.conn, SearchQuery::text("report"))
        .await
        .unwrap();
    let keys: Vec<&str> = result.entries.iter().map(|e| e.entry.key()).collect();
    assert!(keys.contains(&"reports/"));
    assert!(keys.contains(&"projects/2026/report-q3.pdf"));

    let mut q = SearchQuery::text("");
    q.ext = Some("csv".into());
    let r = env.core.search().query(&env.conn, q).await.unwrap();
    assert_eq!(r.entries.len(), 1);

    // アプリ自身の変更は即時に反映する
    let sink = Arc::new(MemorySink::default());
    env.core
        .objects()
        .delete(
            &env.conn,
            vec![Target::file("projects/2026/sales.csv")],
            false,
            sink.clone(),
        )
        .await
        .unwrap();
    wait_batch(&sink).await;
    let mut q = SearchQuery::text("");
    q.ext = Some("csv".into());
    assert!(
        env.core
            .search()
            .query(&env.conn, q)
            .await
            .unwrap()
            .entries
            .is_empty()
    );

    // 外部で削除されたキーは次の走査で消える（世代管理）
    env.s3
        .delete_object()
        .bucket(&env.bucket)
        .key("root.txt")
        .send()
        .await
        .unwrap();
    let sink = Arc::new(MemorySink::default());
    env.core
        .search()
        .rebuild(&env.conn, sink.clone())
        .await
        .unwrap();
    for _ in 0..200 {
        if sink
            .events()
            .iter()
            .any(|e| matches!(e, IndexEvent::Finished { .. }))
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        env.core
            .search()
            .query(&env.conn, SearchQuery::text("root"))
            .await
            .unwrap()
            .entries
            .is_empty()
    );
    let metrics = env.core.metrics_storage(&env.conn, true).await.unwrap();
    assert_eq!(metrics.source, MetricsSource::Index);
    assert_eq!(metrics.object_count, Some(1));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn aborts_multipart_uploads_on_cancel() {
    let endpoint = require_moto!();
    let env = setup(&endpoint, false).await;
    env.core
        .settings_store()
        .patch(
            serde_json::json!({ "transfer": { "multipartThresholdMb": 8, "maxPartsPerFile": 1 } }),
        )
        .unwrap();
    let dir = TempDir::new().unwrap();
    let big = dir.path().join("big.bin");
    // 8 MiB のパートが 20 個。1 つずつ送るため、送信中にキャンセルできる
    std::fs::write(&big, vec![7u8; 160 * 1024 * 1024]).unwrap();
    let selection = env.core.selections().register(vec![big]);
    let plan = env
        .core
        .upload_prepare(&env.conn, "", &selection.selection_id)
        .await
        .unwrap();
    let job = env
        .core
        .upload_start(&plan.plan_id, Decisions::default())
        .await
        .unwrap();

    let multipart_uploads = || async {
        env.s3
            .list_multipart_uploads()
            .bucket(&env.bucket)
            .send()
            .await
            .unwrap()
            .uploads()
            .len()
    };
    // CreateMultipartUpload の後（パートの送信中）にキャンセルする
    let mut started = false;
    for _ in 0..2000 {
        if multipart_uploads().await > 0 {
            started = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(started, "multipart upload did not start");
    assert!(env.core.job_cancel(&job));
    let job = wait_transfer(&env.core, &job).await;
    assert_eq!(job.status, JobStatus::Canceled);

    // 未完了のマルチパートアップロードを中止し、記録も消す（04 §4.2、§14.5）
    assert_eq!(multipart_uploads().await, 0);
    assert!(!env.exists("big.bin").await);
    let records: i64 = env
        .core
        .db()
        .run(|c| Ok(c.query_row("SELECT count(*) FROM transfers", [], |r| r.get(0))?))
        .await
        .unwrap();
    assert_eq!(records, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn deletes_permanently_without_versioning() {
    let endpoint = require_moto!();
    let env = setup(&endpoint, false).await;
    env.put("gone.txt", b"x").await;
    env.put("dir/a.txt", b"x").await;
    env.put("dir/b.txt", b"x").await;

    let sink = Arc::new(MemorySink::default());
    env.core
        .objects()
        .delete(
            &env.conn,
            vec![Target::file("gone.txt"), Target::folder("dir/")],
            false,
            sink.clone(),
        )
        .await
        .unwrap();
    let r = wait_batch(&sink).await;
    assert_eq!(r.succeeded, 3);
    assert!(env.keys("").await.is_empty());

    // バージョニングが無効のバケットでは完全に削除され、削除マーカーも残らない（04 §6.1）
    let versions = env
        .s3
        .list_object_versions()
        .bucket(&env.bucket)
        .send()
        .await
        .unwrap();
    assert!(versions.versions().is_empty());
    assert!(versions.delete_markers().is_empty());
    let deleted = env
        .list(
            "",
            ListOptions {
                show_hidden: false,
                include_deleted: true,
            },
        )
        .await;
    assert!(deleted.is_empty(), "{deleted:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn requests_restores_and_detects_completion() {
    let endpoint = require_moto!();
    let env = setup(&endpoint, false).await;
    env.s3
        .put_object()
        .bucket(&env.bucket)
        .key("cold/archive.bin")
        .storage_class(aws_sdk_s3::types::StorageClass::Glacier)
        .body(ByteStream::from_static(b"frozen"))
        .send()
        .await
        .unwrap();
    env.put("cold/warm.txt", b"warm").await;

    let detail = env
        .core
        .objects()
        .head(&env.conn, "cold/archive.bin", None)
        .await
        .unwrap();
    assert_eq!(detail.restore, RestoreState::Archived);

    // フォルダを指定すると、配下のアーカイブだけを取り出す（04 §8.4）
    let result = env
        .core
        .objects()
        .request_restore(
            &env.conn,
            &[Target::folder("cold/")],
            RestoreTier::Standard,
            Some(3),
        )
        .await
        .unwrap();
    assert_eq!(result.succeeded, 1, "{result:?}");
    assert!(result.failed.is_empty());

    // moto は取り出しをすぐに完了させる。完了した要求だけを知らせる
    let completed = env.core.objects().check_restores().await.unwrap();
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].key, "cold/archive.bin");
    assert!(
        env.core
            .objects()
            .check_restores()
            .await
            .unwrap()
            .is_empty()
    );
    let detail = env
        .core
        .objects()
        .head(&env.conn, "cold/archive.bin", None)
        .await
        .unwrap();
    assert!(matches!(detail.restore, RestoreState::Restored { .. }));
}

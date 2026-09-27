//! S3 Drive のコア（Tauri 非依存。05 §1）。
//!
//! ドメインモデル、Google 認証、認証情報、AWS 操作、転送、検索、メトリクス、SQLite、設定、ジョブを持つ。
//! `src-tauri` はこのクレートの [`Core`] を IPC コマンドから呼び出すだけにする。

pub mod auth;
pub mod aws;
pub mod connections;
pub mod credentials;
pub mod error;
pub mod jobs;
pub mod metrics;
pub mod model;
pub mod objects;
pub mod search;
pub mod selection;
pub mod store;
pub mod transfer;
pub mod util;
pub mod versions;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

pub use error::{AppError, CoreError, CoreResult, ErrorCode};

use auth::AuthProvider;
use connections::ConnCtx;
use jobs::JobRegistry;
use model::{ConnectionId, UserSession};
use selection::SelectionRegistry;
use store::{Db, SecretStore, SettingsStore};
use transfer::TransferManager;

/// [`Core`] の生成に必要な設定。
pub struct CoreConfig {
    /// `settings.json` と `s3drive.db` を置くフォルダ（`~/Library/Application Support/{バンドル ID}`）。
    pub data_dir: PathBuf,
    pub secrets: Arc<dyn SecretStore>,
    pub auth: Arc<dyn AuthProvider>,
    /// 全接続のエンドポイントの上書き（結合テスト・E2E で moto を使う場合）。
    pub endpoint_override: Option<String>,
    /// 接続ごとの `endpointUrl` を有効にするか（開発ビルドのみ。06 §2）。
    pub allow_connection_endpoints: bool,
}

pub(crate) struct CoreInner {
    pub(crate) session: RwLock<Option<UserSession>>,
    pub(crate) settings: SettingsStore,
    pub(crate) secrets: Arc<dyn SecretStore>,
    pub(crate) db: Db,
    pub(crate) clients: Mutex<HashMap<ConnectionId, Arc<ConnCtx>>>,
    pub(crate) jobs: JobRegistry,
    pub(crate) selections: SelectionRegistry,
    pub(crate) auth: Arc<dyn AuthProvider>,
    pub(crate) transfers: TransferManager,
    pub(crate) index_builds: search::IndexBuilds,
    pub(crate) index_listener: Mutex<Option<Box<search::IndexListener>>>,
    pub(crate) sign_in_cancel: Mutex<Option<tokio_util::sync::CancellationToken>>,
    pub(crate) endpoint_override: Option<String>,
    pub(crate) allow_connection_endpoints: bool,
    pub(crate) db_recreated: bool,
}

/// サービス群のファサード（05 §1.3）。複製しても同じ状態を共有する。
#[derive(Clone)]
pub struct Core(pub(crate) Arc<CoreInner>);

impl Core {
    pub fn new(config: CoreConfig) -> CoreResult<Self> {
        let settings = SettingsStore::load(&config.data_dir.join("settings.json"))?;
        let opened = Db::open(&config.data_dir.join("s3drive.db"))?;
        Ok(Self::assemble(
            config,
            settings,
            opened.db,
            opened.recreated,
        ))
    }

    fn assemble(config: CoreConfig, settings: SettingsStore, db: Db, db_recreated: bool) -> Self {
        Self(Arc::new(CoreInner {
            session: RwLock::new(None),
            settings,
            secrets: config.secrets,
            transfers: TransferManager::new(db.clone()),
            db,
            clients: Mutex::new(HashMap::new()),
            jobs: JobRegistry::new(),
            selections: SelectionRegistry::new(),
            auth: config.auth,
            index_builds: search::IndexBuilds::default(),
            index_listener: Mutex::new(None),
            sign_in_cancel: Mutex::new(None),
            endpoint_override: config.endpoint_override,
            allow_connection_endpoints: config.allow_connection_endpoints,
            db_recreated,
        }))
    }

    /// テスト用。一時フォルダ・メモリ上のキーチェーン・固定のサインインで作る。
    pub fn for_tests(
        endpoint_override: Option<String>,
    ) -> CoreResult<(Self, store::db::tempfile_guard::TempDir)> {
        let dir = store::db::tempfile_guard::TempDir::new()?;
        let core = Self::new(CoreConfig {
            data_dir: dir.path().to_path_buf(),
            secrets: Arc::new(store::MemorySecretStore::new()),
            auth: Arc::new(auth::FixedAuth::test_user()),
            endpoint_override,
            allow_connection_endpoints: true,
        })?;
        Ok((core, dir))
    }

    /// SQLite を作り直した（起動時にトーストで知らせる。06 §6）。
    pub fn db_recreated(&self) -> bool {
        self.0.db_recreated
    }

    pub fn db(&self) -> &Db {
        &self.0.db
    }

    pub fn jobs(&self) -> &JobRegistry {
        &self.0.jobs
    }

    /// 転送・一括操作・インデックス作成のキャンセル（`job_cancel`）。
    pub fn job_cancel(&self, job_id: &str) -> bool {
        self.0.transfers.cancel(job_id) || self.0.jobs.cancel(job_id)
    }

    pub fn selections(&self) -> &SelectionRegistry {
        &self.0.selections
    }

    pub fn settings_store(&self) -> &SettingsStore {
        &self.0.settings
    }

    pub fn transfers(&self) -> &TransferManager {
        &self.0.transfers
    }

    /// サインイン中のセッション。
    pub fn session(&self) -> Option<UserSession> {
        self.0.session.read().unwrap().clone()
    }

    /// サインイン中のアカウント（Google の `sub`）。未サインインなら `AUTH_REQUIRED`。
    pub(crate) fn account(&self) -> CoreResult<UserSession> {
        self.session()
            .ok_or_else(|| CoreError::new(ErrorCode::AuthRequired))
    }

    pub(crate) fn set_session(&self, session: Option<UserSession>) {
        *self.0.session.write().unwrap() = session;
    }

    /// セッション・クライアント・ジョブをまとめて破棄する（05 §1.3）。
    pub(crate) fn reset_runtime_state(&self) {
        self.0.jobs.cancel_all();
        self.0.transfers.cancel_all();
        self.0.clients.lock().unwrap().clear();
        self.set_session(None);
    }
}

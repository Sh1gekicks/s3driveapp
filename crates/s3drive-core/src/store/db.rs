//! SQLite（06 §4）。WAL・コネクションプール・`spawn_blocking` 上での実行。

use std::path::{Path, PathBuf};

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, TransactionBehavior};
use rusqlite_migration::{M, Migrations};

use crate::error::{CoreError, CoreResult};

const MIGRATIONS: &[M<'static>] = &[M::up(include_str!("migrations.sql"))];

#[derive(Clone)]
pub struct Db {
    pool: Pool<SqliteConnectionManager>,
    path: Option<PathBuf>,
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Db").field("path", &self.path).finish()
    }
}

/// `Db::open` の結果。壊れていたファイルを作り直した場合は `recreated` が真になる（06 §6）。
pub struct OpenedDb {
    pub db: Db,
    pub recreated: bool,
}

fn init_connection(conn: &mut Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;
         PRAGMA synchronous = NORMAL;",
    )?;
    // トランザクションは書き込みのために使うため、開始時に書き込みロックを取る。既定の DEFERRED では、
    // 読み取りから書き込みに移るとき（接続が初めて FTS5 の表に触れて設定を読む場合を含む）に他の接続が
    // 書き込み中だと、busy_timeout で待たずに SQLITE_BUSY になる
    conn.set_transaction_behavior(TransactionBehavior::Immediate);
    // 検索結果を一覧と同じ自然順（数字は数値として比べる）で並べる（04 §10.2）
    conn.create_collation("NATURAL_ORDER", crate::util::natural_cmp)
}

/// ベンチマーク用（09 §5）。マイグレーションを適用した接続を開く。
#[cfg(feature = "bench")]
pub(crate) fn open_for_bench(path: &Path) -> Connection {
    let mut conn = Connection::open(path).unwrap();
    init_connection(&mut conn).unwrap();
    Migrations::from_slice(MIGRATIONS)
        .to_latest(&mut conn)
        .unwrap();
    conn
}

impl Db {
    /// ファイルを開き、マイグレーションを適用する。開けない場合は退避して作り直す。
    pub fn open(path: &Path) -> CoreResult<OpenedDb> {
        match Self::try_open(path) {
            Ok(db) => Ok(OpenedDb {
                db,
                recreated: false,
            }),
            Err(err) => {
                log::warn!("SQLite を開けないため作り直します: {err}");
                let backup = path.with_extension(format!(
                    "db.corrupt-{}",
                    chrono::Utc::now().format("%Y%m%d%H%M%S")
                ));
                if path.exists() {
                    std::fs::rename(path, &backup).map_err(|e| CoreError::local_io(path, &e))?;
                }
                for suffix in ["-wal", "-shm"] {
                    let side = PathBuf::from(format!("{}{suffix}", path.display()));
                    let _ = std::fs::remove_file(side);
                }
                let db = Self::try_open(path)?;
                Ok(OpenedDb {
                    db,
                    recreated: true,
                })
            }
        }
    }

    fn try_open(path: &Path) -> CoreResult<Db> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| CoreError::local_io(dir, &e))?;
        }
        // 壊れたファイルは最初のクエリで検出される（プールを作る前に確認し、接続の待ち時間を避ける）
        {
            let mut conn = Connection::open(path)?;
            init_connection(&mut conn)?;
            conn.query_row("PRAGMA schema_version", [], |r| r.get::<_, i64>(0))?;
            Migrations::from_slice(MIGRATIONS)
                .to_latest(&mut conn)
                .map_err(|e| CoreError::internal(format!("migration: {e}")))?;
        }
        let manager = SqliteConnectionManager::file(path).with_init(init_connection);
        let pool = Pool::builder()
            .max_size(8)
            .connection_timeout(std::time::Duration::from_secs(10))
            .build(manager)?;
        Ok(Db {
            pool,
            path: Some(path.to_path_buf()),
        })
    }

    /// テスト用。一時ファイルの DB を作る。
    pub fn open_temp() -> CoreResult<(Db, tempfile_guard::TempDir)> {
        let dir = tempfile_guard::TempDir::new()?;
        let db = Self::try_open(&dir.path().join("s3drive.db"))?;
        Ok((db, dir))
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// DB ファイルの大きさ（WAL を含む）。
    pub fn size_bytes(&self) -> u64 {
        let Some(path) = &self.path else { return 0 };
        ["", "-wal"]
            .iter()
            .filter_map(|s| std::fs::metadata(format!("{}{s}", path.display())).ok())
            .map(|m| m.len())
            .sum()
    }

    /// ブロッキングスレッドで処理を実行する。
    pub async fn run<T, F>(&self, f: F) -> CoreResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> CoreResult<T> + Send + 'static,
    {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            f(&mut conn)
        })
        .await?
    }

    /// 同期的に実行する（起動時の初期化など）。
    pub fn run_blocking<T>(
        &self,
        f: impl FnOnce(&mut Connection) -> CoreResult<T>,
    ) -> CoreResult<T> {
        let mut conn = self.pool.get()?;
        f(&mut conn)
    }
}

/// テスト・開発用の一時ディレクトリ（`tempfile` を本体の依存にしないための最小実装）。
pub mod tempfile_guard {
    use std::path::{Path, PathBuf};

    use crate::error::{CoreError, CoreResult};

    #[derive(Debug)]
    pub struct TempDir(PathBuf);

    impl TempDir {
        pub fn new() -> CoreResult<Self> {
            let dir = std::env::temp_dir().join(format!("s3drive-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).map_err(|e| CoreError::local_io(&dir, &e))?;
            Ok(Self(dir))
        }

        pub fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn applies_migrations_and_supports_fts() {
        let (db, _dir) = Db::open_temp().unwrap();
        let count = db
            .run(|c| {
                c.execute(
                    "INSERT INTO objects (connection_id, key, name, name_norm, ext, parent, size, last_modified, storage_class, generation)
                     VALUES ('c', 'a/report-q3.pdf', 'report-q3.pdf', 'report-q3.pdf', 'pdf', 'a/', 1, '2026-01-01T00:00:00Z', 'STANDARD', 1)",
                    [],
                )?;
                Ok(c.query_row(
                    "SELECT count(*) FROM objects_fts WHERE objects_fts MATCH '\"port\"'",
                    [],
                    |r| r.get::<_, i64>(0),
                )?)
            })
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn write_transactions_wait_for_other_writers() {
        let (db, _dir) = Db::open_temp().unwrap();
        let mut writer = db.pool.get().unwrap();
        // まだ FTS5 の表に触れていない接続
        let mut fresh = db.pool.get().unwrap();
        let (locked_tx, locked_rx) = std::sync::mpsc::channel();
        std::thread::scope(|s| {
            s.spawn(move || {
                let tx = writer.transaction().unwrap();
                tx.execute(
                    "INSERT INTO index_state (connection_id, status, generation) VALUES ('a', 'building', 1)",
                    [],
                )
                .unwrap();
                locked_tx.send(()).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(200));
                tx.commit().unwrap();
            });
            locked_rx.recv().unwrap();
            // 書き込み中の接続があっても、SQLITE_BUSY にならずに待って書き込める
            let tx = fresh.transaction().unwrap();
            tx.execute(
                "INSERT INTO objects (connection_id, key, name, name_norm, ext, parent, size, last_modified, storage_class, generation)
                 VALUES ('b', 'x.txt', 'x.txt', 'x.txt', 'txt', '', 1, '2026-01-01T00:00:00Z', 'STANDARD', 1)",
                [],
            )
            .unwrap();
            tx.commit().unwrap();
        });
    }

    #[test]
    fn recreates_a_corrupt_file() {
        let dir = tempfile_guard::TempDir::new().unwrap();
        let path = dir.path().join("s3drive.db");
        std::fs::write(
            &path,
            b"this is not a sqlite database at all, just garbage bytes",
        )
        .unwrap();
        let opened = Db::open(&path).unwrap();
        assert!(opened.recreated);
        let reopened = Db::open(&path).unwrap();
        assert!(!reopened.recreated);
    }
}

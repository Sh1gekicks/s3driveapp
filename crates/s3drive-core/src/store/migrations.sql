-- s3drive.db の初版スキーマ（06 §4）

CREATE TABLE index_state (
  connection_id     TEXT PRIMARY KEY,
  status            TEXT NOT NULL,              -- building / ready
  generation        INTEGER NOT NULL,
  object_count      INTEGER NOT NULL DEFAULT 0,
  total_bytes       INTEGER NOT NULL DEFAULT 0,
  last_full_scan_at TEXT                        -- RFC 3339
);

CREATE TABLE objects (
  id            INTEGER PRIMARY KEY,            -- FTS5 の content_rowid に使う
  connection_id TEXT NOT NULL,
  key           TEXT NOT NULL,
  name          TEXT NOT NULL,                  -- 表示名（最後の階層、NFC）
  name_norm     TEXT NOT NULL,                  -- 検索用（NFKC + 小文字）
  ext           TEXT,                           -- 小文字の拡張子
  parent        TEXT NOT NULL,                  -- 親プレフィックス
  size          INTEGER NOT NULL,
  last_modified TEXT NOT NULL,
  etag          TEXT,
  storage_class TEXT NOT NULL,
  is_marker     INTEGER NOT NULL DEFAULT 0,     -- フォルダマーカー
  generation    INTEGER NOT NULL,
  UNIQUE (connection_id, key)
);
CREATE INDEX objects_parent   ON objects (connection_id, parent);
CREATE INDEX objects_ext      ON objects (connection_id, ext);
CREATE INDEX objects_modified ON objects (connection_id, last_modified);
CREATE INDEX objects_size     ON objects (connection_id, size);
CREATE INDEX objects_class    ON objects (connection_id, storage_class);

CREATE VIRTUAL TABLE objects_fts USING fts5(
  name_norm,
  content = 'objects',
  content_rowid = 'id',
  tokenize = 'trigram'
);

CREATE TRIGGER objects_ai AFTER INSERT ON objects BEGIN
  INSERT INTO objects_fts (rowid, name_norm) VALUES (new.id, new.name_norm);
END;
CREATE TRIGGER objects_ad AFTER DELETE ON objects BEGIN
  INSERT INTO objects_fts (objects_fts, rowid, name_norm) VALUES ('delete', old.id, old.name_norm);
END;
CREATE TRIGGER objects_au AFTER UPDATE OF name_norm ON objects BEGIN
  INSERT INTO objects_fts (objects_fts, rowid, name_norm) VALUES ('delete', old.id, old.name_norm);
  INSERT INTO objects_fts (rowid, name_norm) VALUES (new.id, new.name_norm);
END;

CREATE TABLE prefixes (
  id            INTEGER PRIMARY KEY,
  connection_id TEXT NOT NULL,
  prefix        TEXT NOT NULL,
  name          TEXT NOT NULL,
  name_norm     TEXT NOT NULL,
  parent        TEXT NOT NULL,
  generation    INTEGER NOT NULL,
  UNIQUE (connection_id, prefix)
);
CREATE INDEX prefixes_parent ON prefixes (connection_id, parent);

CREATE TABLE transfers (
  id            TEXT PRIMARY KEY,
  job_id        TEXT NOT NULL,
  connection_id TEXT NOT NULL,
  direction     TEXT NOT NULL,                  -- upload / download
  key           TEXT NOT NULL,
  local_path    TEXT NOT NULL,
  size          INTEGER NOT NULL,
  done_bytes    INTEGER NOT NULL DEFAULT 0,
  status        TEXT NOT NULL,
  upload_id     TEXT,
  part_size     INTEGER
);
CREATE INDEX transfers_connection ON transfers (connection_id);

CREATE TABLE transfer_parts (
  transfer_id TEXT NOT NULL REFERENCES transfers (id) ON DELETE CASCADE,
  part_number INTEGER NOT NULL,
  etag        TEXT NOT NULL,
  checksum    TEXT,
  PRIMARY KEY (transfer_id, part_number)
);

CREATE TABLE restore_requests (
  connection_id TEXT NOT NULL,
  key           TEXT NOT NULL,
  version_id    TEXT NOT NULL DEFAULT '',
  tier          TEXT NOT NULL,
  days          INTEGER,
  status        TEXT NOT NULL,                  -- inProgress / restored / abandoned
  requested_at  TEXT NOT NULL,
  expiry_at     TEXT,
  PRIMARY KEY (connection_id, key, version_id)
);

CREATE TABLE metrics_cache (
  connection_id TEXT NOT NULL,                  -- 単価など接続に依存しないものは ''
  kind          TEXT NOT NULL,                  -- storage / cost / pricing:ap-northeast-1
  payload       TEXT NOT NULL,                  -- JSON
  fetched_at    TEXT NOT NULL,
  expires_at    TEXT,                           -- NULL は期限なし（コスト。04 §13.4）
  PRIMARY KEY (connection_id, kind)
);

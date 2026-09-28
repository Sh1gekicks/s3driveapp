# 05. バックエンドと IPC

## 1. Rust のモジュール構成

### 1.1 クレートの責務

| クレート | 責務 | 依存してよいもの |
|---|---|---|
| `crates/s3drive-core` | ドメインモデル、Google 認証、認証情報、AWS 操作、転送、検索、メトリクス、SQLite、設定、ジョブ | AWS SDK、tokio、rusqlite、keyring-core など。**Tauri には依存しない** |
| `src-tauri` | IPC コマンド、Tauri プラグインの登録、ウィンドウ・メニュー・トレイ、ドラッグ＆ドロップ、`Channel` とコアの進捗通知の橋渡し | `s3drive-core`、tauri と各プラグイン |

コアはジョブの進捗を `ProgressSink` トレイトで通知する。`src-tauri` はこれを `tauri::ipc::Channel` で実装し、テストではメモリ上の実装に差し替える。

```rust
// s3drive-core
pub trait ProgressSink<E>: Send + Sync + 'static {
    fn send(&self, event: E);
}

// src-tauri
impl<E: serde::Serialize + Clone + Send + Sync + 'static> ProgressSink<E> for ChannelSink<E> {
    fn send(&self, event: E) {
        let _ = self.0.send(event); // 受信側が閉じていても処理は継続する
    }
}
```

### 1.2 s3drive-core のモジュール

| モジュール | 主な型・関数 | 役割 |
|---|---|---|
| `model` | `UserSession`、`Connection`、`Entry`、`ObjectDetail`、`ObjectVersion`、`StorageClass`、`RestoreState`、`BucketInfo`、`TransferJob`、`SearchQuery`、`StorageMetrics`、`CostSummary` | IPC で受け渡す DTO（serde + ts-rs） |
| `auth` | `GoogleAuth::{sign_in, restore, sign_out}`、`LoopbackServer` | Google の OAuth（[07 §2](07-security.md#2-google-認証oauth-20--pkce)） |
| `credentials` | `AccessKey`、`AssumeRoleCredentials`、`ExpiredTokenRetry` | キーチェーンに保存する値と、静的キー／AssumeRole の認証情報プロバイダ、`ExpiredToken` の再試行（[01 §7.1](01-architecture.md#71-エラーハンドリング)） |
| `aws` | `sdk_config()`、`Clients`、`error_map::classify()` | SDK の設定と接続ごとのクライアント群、エラー分類 |
| `connections` | `ConnectionService::{list, test, create, update, patch, delete, reorder, credential_list, credential_update, bucket_info}`、`ConnCtx` | 接続・認証情報の管理。接続ごとのクライアントとバケット情報を `ConnCtx` にまとめ、`Core` が接続 ID ごとにキャッシュする |
| `objects` | `ObjectService::{list_page, head, create_folder, folder_summary, folder_children, find_conflicts, delete, move_objects, rename, change_storage_class, request_restore, check_restores}` | オブジェクト・フォルダ操作 |
| `versions` | `VersionService::{list, restore, delete, undelete}` | バージョン管理 |
| `transfer` | `Core::{upload_prepare, upload_start, download_start, transfer_retry, abort_stale_uploads}`、`TransferManager::{subscribe, enqueue, cancel, shutdown}`、`multipart` | 転送キューと実行 |
| `search` | `IndexRunner`（全件走査）、`upsert`／`remove`（アプリ自身の変更の反映）、`SearchService::{query, status, rebuild, delete}` | 検索インデックス |
| `metrics` | `StorageMetricsService`（CloudWatch）、`CostService`（Cost Explorer）、`PricingService` | 容量・コスト・単価 |
| `jobs` | `JobRegistry`、`JobHandle`、`JobId` | ジョブの登録・キャンセル |
| `selection` | `SelectionRegistry` | ファイル選択・ドロップで得たローカルパスの保管（§3.9） |
| `store` | `Db`（コネクションプール）、`migrations.sql`、`SettingsStore`、`SecretStore`（`KeyringSecretStore`／テスト用の `MemorySecretStore`）、`metrics_cache` | 永続化（[06-data.md](06-data.md)） |
| `util` | `key`（正規化・検証・結合）、`time`、`region`（リージョン名）、`natural_cmp`（自然順の比較） | 共通処理 |

### 1.3 アプリの状態

```rust
pub struct AppState {
    pub core: s3drive_core::Core, // サービス群のファサード
    pub quitting: AtomicBool,     // 終了の確認を済ませた
}

#[derive(Clone)]
pub struct Core(Arc<CoreInner>); // 複製しても同じ状態を共有する

struct CoreInner {
    session: RwLock<Option<UserSession>>,
    clients: Mutex<HashMap<ConnectionId, Arc<ConnCtx>>>, // 接続ごとのクライアント
    jobs: JobRegistry,
    selections: SelectionRegistry,
    transfers: TransferManager,
    db: Db,
    settings: SettingsStore,
    secrets: Arc<dyn SecretStore>,
    auth: Arc<dyn AuthProvider>, // Google、または E2E・開発用の固定のセッション
    // ほかにインデックスの走査状態、サインインのキャンセルなど
}
```

サインアウトやアカウントの切り替えでは、`Core` がセッション・クライアント・ジョブをまとめて破棄する。

## 2. 型定義（DTO）

Rust の DTO から ts-rs で TypeScript の型を生成する（§6）。以下は生成される型の要約である。

```ts
type ConnectionId = string; // UUID
type JobId = string;        // UUID
type Timestamp = string;    // RFC 3339（UTC）

type StorageClass =
  | 'STANDARD' | 'INTELLIGENT_TIERING' | 'STANDARD_IA' | 'ONEZONE_IA'
  | 'GLACIER_IR' | 'GLACIER' | 'DEEP_ARCHIVE' | 'OTHER';
type Versioning = 'enabled' | 'suspended' | 'disabled' | 'unknown';
type FileKind = 'image' | 'video' | 'audio' | 'pdf' | 'sheet' | 'archive' | 'code' | 'doc';

interface UserSession { sub: string; email: string; name: string; initial: string }

interface Connection {
  id: ConnectionId;
  bucket: string;
  region: string;               // ap-northeast-1
  regionLabel: string;          // アジアパシフィック (東京)
  regionShort: string;          // 東京
  credentialId: string;
  accessKeyIdMasked: string;    // AKIA************7Q2LM
  roleArn: string | null;
  externalId: string | null;
  useSourceIdentity: boolean;
  costTag: { key: string; value: string } | null;
  defaultStorageClass: StorageClass | null;
}

interface ConnectionInput {
  bucket: string;
  region: string;
  credential:
    | { kind: 'new'; accessKeyId: string; secretAccessKey: string }
    | { kind: 'existing'; credentialId: string };
  roleArn?: string;
  externalId?: string;
}

interface ConnectionTestResult {
  accountId: string; callerArn: string;
  region: string; regionCorrected: boolean; versioning: Versioning;
}

interface BucketInfo {
  bucket: string; region: string; versioning: Versioning; encryption: string;
  regionCorrected: boolean;     // HeadBucket で接続のリージョンを修正した（01 §6.1）
}

type RestoreState =
  | { state: 'notArchived' }
  | { state: 'archived' }                        // 取り出しが必要
  | { state: 'inProgress' }
  | { state: 'restored'; expiry: Timestamp };

type Entry =
  | { type: 'folder'; key: string; name: string; lastModified: Timestamp | null; deleted: boolean }
  | { type: 'file'; key: string; name: string; size: number; lastModified: Timestamp;
      etag: string; storageClass: StorageClass; restore: RestoreState; deleted: boolean };

interface ListPage { entries: Entry[]; nextToken: string | null }

interface ObjectDetail {
  key: string; versionId: string | null; size: number; contentType: string;
  lastModified: Timestamp;
  created: { at: Timestamp; source: 'oldestVersion' | 'metadata' | 'lastModified' };
  etag: string; storageClass: StorageClass; encryption: string; kmsKeyId: string | null;
  checksums: Record<string, string>; userMetadata: Record<string, string>;
  restore: RestoreState;
}

interface ObjectVersion {
  versionId: string; isLatest: boolean; isDeleteMarker: boolean;
  lastModified: Timestamp; size: number | null; storageClass: StorageClass | null; etag: string | null;
}

interface Target { key: string; isFolder: boolean; versionId?: string }

interface BatchResult {
  succeeded: number;            // 項目（キー）の数。全バージョンの削除でもバージョンではなくキーで数える
  skipped: { key: string; reason: string }[];
  failed: { key: string; error: AppError }[];
}

type BatchEvent =
  | { event: 'progress'; data: { jobId: JobId; done: number; total: number | null } }
  | { event: 'finished'; data: { jobId: JobId; result: BatchResult } };

interface Selection { selectionId: string; items: { name: string; size: number; isDir: boolean }[] }

interface UploadPlan {
  planId: string; fileCount: number; totalBytes: number;
  versioningEnabled: boolean;   // DLG-08 の説明文
  conflicts: { key: string; localSize: number; remoteSize: number; remoteModified: Timestamp }[];
  excluded: { name: string; reason: 'symlink' | 'ignored' | 'keyTooLong' | 'invalidChar' | 'unreadable' }[];
}
type ConflictDecision = 'replace' | 'skip' | 'keepBoth';

interface TransferJob {
  jobId: JobId; kind: 'upload' | 'download'; connectionId: ConnectionId; title: string;
  status: 'queued' | 'running' | 'succeeded' | 'failed' | 'canceled';
  totalFiles: number; doneFiles: number; failedFiles: number;
  totalBytes: number; doneBytes: number; currentName: string | null;
  bytesPerSec: number; etaSec: number | null;
  destination: string;          // 「{バケット}/{プレフィックス}」または保存先フォルダ
}

type TransferEvent =
  | { event: 'jobUpdated'; data: TransferJob }
  | { event: 'fileFailed'; data: { jobId: JobId; name: string; error: AppError } };

interface SearchQuery {
  text: string; kind?: FileKind; ext?: string;
  size?: 'lt1' | '1to100' | 'gt100'; date?: '7d' | '30d' | 'year';
  storageClass?: StorageClass;
  sort: { key: 'name' | 'modified' | 'size' | 'storageClass'; dir: 1 | -1 };
  offset: number; limit: number;
}
interface IndexStatus {
  state: 'none' | 'building' | 'ready' | 'stale';
  objectCount: number; lastScanAt: Timestamp | null; sizeBytes: number; progress: number | null;
}
// フォルダは最初のページ（offset = 0）の先頭にだけ最大 200 件を含め、offset・limit はファイルに適用する（04 §10.2）
interface SearchResult { entries: { entry: Entry; parent: string }[]; total: number; index: IndexStatus }

interface StorageMetrics {
  source: 'cloudwatch' | 'index' | 'none'; asOf: Timestamp | null;
  totalBytes: number; objectCount: number | null;
  byClass: Partial<Record<StorageClass, number>>;
}
interface CostSummary {
  scope: { kind: 'tag'; key: string; value: string } | { kind: 'accountRegion'; region: string };
  month: string;                     // 2026-09
  monthToDate: number;
  prevMonthSamePeriod: number | null;
  breakdown: { storage: number; requests: number; transfer: number; retrieval: number; other: number };
  daily: (number | null)[];          // 当月の日数分。未来日は null
  forecastMonthEnd: number | null;
  currency: 'USD'; fetchedAt: Timestamp;
}
interface PriceTable {
  region: string; perGbMonth: Partial<Record<StorageClass, number>>;
  source: 'api' | 'bundled'; asOf: Timestamp;
}

interface AppError { code: ErrorCode; message: string; detail?: string; retryable: boolean }
```

- 日時は RFC 3339（UTC）の文字列で受け渡し、表示時にローカル時刻へ変換する。
- 64 ビット整数（サイズ・件数）は `#[ts(type = "number")]` で number として生成する（2^53 未満であることを前提とする）。
- 設定（`Settings`）の型は [06 §2](06-data.md#2-設定ファイル) を参照する。

## 3. コマンド一覧

コマンド名は Rust の関数名（スネークケース）。引数は JavaScript からキャメルケースで渡す（Tauri が変換する）。すべて `Result<T, AppError>` を返す。

### 3.1 認証

| コマンド | 引数 | 戻り値 | 処理 |
|---|---|---|---|
| `auth_get_session` | — | `UserSession \| null` | メモリ上のセッションを返す |
| `auth_restore` | — | `UserSession \| null` | 起動時にリフレッシュトークンからセッションを復元する（[04 §1.3](04-features.md#13-起動時のセッション復元)） |
| `auth_sign_in` | — | `UserSession` | ブラウザでサインインし、完了まで待つ（最大 5 分） |
| `auth_cancel_sign_in` | — | — | 待ち受けを中断する |
| `auth_sign_out` | — | — | トークンの取り消し、ジョブのキャンセル、状態の破棄 |

### 3.2 接続・認証情報

| コマンド | 引数 | 戻り値 | 処理 |
|---|---|---|---|
| `connection_list` | — | `Connection[]` | サインイン中のアカウントの接続 |
| `connection_test` | `input: ConnectionInput` | `ConnectionTestResult` | [04 §2.2](04-features.md#22-接続の確認) の確認 |
| `connection_create` | `input` | `Connection` | 確認済みの入力を保存する |
| `connection_update` | `id`、`input` | `Connection` | 再確認して保存する |
| `connection_patch` | `id`、`patch: { costTag?, defaultStorageClass?, useSourceIdentity? }` | `Connection` | 接続ごとの設定（コスト配分タグなど。確認は不要） |
| `connection_delete` | `id` | — | 接続と関連データを削除する |
| `connection_reorder` | `ids: ConnectionId[]` | — | サイドバーの順序 |
| `connection_last_location` | — | `{ connectionId, prefix } \| null` | 前回表示していた場所（起動時に復元する。03 §2） |
| `connection_set_location` | `location` | — | 表示中の場所を保存する |
| `credential_list` | — | `{ id, accessKeyIdMasked, usedBy: string[] }[]` | DLG-05 の既存の認証情報 |
| `credential_update` | `credentialId`、`accessKeyId`、`secretAccessKey` | — | 確認してキーチェーンを更新する |
| `bucket_get_info` | `connectionId`、`force?` | `BucketInfo` | リージョン・バージョニング・暗号化。リージョンが違えば修正し、`connections://changed` を送る（[01 §6.1](01-architecture.md#61-aws-クライアントの管理)） |

### 3.3 オブジェクト・フォルダ

| コマンド | 引数 | 戻り値 | 処理 |
|---|---|---|---|
| `objects_list_page` | `connectionId`、`prefix`、`token`、`opts: { showHidden, includeDeleted }` | `ListPage` | [04 §3](04-features.md#3-ファイル一覧フォルダ表示) |
| `object_head` | `connectionId`、`key`、`versionId?` | `ObjectDetail` | [04 §9](04-features.md#9-メタデータ表示) |
| `folder_create` | `connectionId`、`prefix`、`name` | `Entry` | フォルダマーカーの作成 |
| `folder_summary` | `connectionId`、`prefix` | `{ itemCount, totalBytes, truncated }` | 項目数・合計サイズ |
| `folder_children` | `connectionId`、`prefix` | `Entry[]`（フォルダのみ） | 移動ダイアログのツリー |
| `objects_find_conflicts` | `connectionId`、`targets`、`destPrefix` | `{ key, remoteSize, remoteModified }[]` | 移動先の同名チェック |
| `objects_delete` | `connectionId`、`targets`、`allVersions`、`onEvent: Channel<BatchEvent>` | `JobId` | [04 §6](04-features.md#6-削除) |
| `objects_move` | `connectionId`、`targets`、`destPrefix`、`decisions`、`onEvent` | `JobId` | [04 §7.3](04-features.md#73-移動名前の変更を含む) |
| `object_rename` | `connectionId`、`target`、`newName`、`onEvent` | `JobId` | 移動として処理する |
| `objects_change_storage_class` | `connectionId`、`targets`、`storageClass`、`onEvent` | `JobId` | [04 §8.2](04-features.md#82-変更の処理) |
| `objects_request_restore` | `connectionId`、`targets`、`tier`、`days?` | `BatchResult` | [04 §8.4](04-features.md#84-アーカイブの取り出し) |

### 3.4 バージョン

| コマンド | 引数 | 戻り値 | 処理 |
|---|---|---|---|
| `versions_list` | `connectionId`、`key`、`cursor?` | `{ versions: ObjectVersion[], nextCursor }` | [04 §11.2](04-features.md#112-バージョン一覧) |
| `version_restore` | `connectionId`、`key`、`versionId` | `ObjectVersion` | 以前のバージョンを最新にする |
| `version_delete` | `connectionId`、`key`、`versionId` | — | バージョンの完全削除 |
| `deleted_restore` | `connectionId`、`keys` | `BatchResult` | 削除マーカーを取り除く |

### 3.5 転送

| コマンド | 引数 | 戻り値 | 処理 |
|---|---|---|---|
| `transfer_subscribe` | `onEvent: Channel<TransferEvent>` | `TransferJob[]` | 起動時に 1 回呼び、現在のジョブ一覧を受け取ってから以後の更新を購読する |
| `pick_upload_files` | `directories: boolean` | `Selection \| null` | Rust 側でファイル（またはフォルダ）選択ダイアログを開く |
| `upload_prepare` | `connectionId`、`prefix`、`selectionId` | `UploadPlan` | [04 §4.1](04-features.md#41-処理の流れ) の準備 |
| `upload_start` | `planId`、`decisions: Record<string, ConflictDecision> \| { all: ConflictDecision }` | `JobId` | アップロードの開始 |
| `pick_download_dir` | — | `Selection \| null` | 保存先フォルダの選択ダイアログ |
| `download_start` | `connectionId`、`targets`、`destination: 'default' \| { selectionId }` | `JobId` | [04 §5](04-features.md#5-ダウンロード) |
| `download_reveal` | `jobId` | — | 保存したファイルを Finder で表示する |
| `job_cancel` | `jobId` | — | 転送・一括操作・インデックス作成のキャンセル |
| `transfer_retry` | `jobId` | `JobId` | 失敗したファイルだけを再実行する |
| `transfer_clear_finished` | — | — | 完了済みのジョブを一覧から消す |

### 3.6 検索

| コマンド | 引数 | 戻り値 | 処理 |
|---|---|---|---|
| `search_query` | `connectionId`、`query: SearchQuery` | `SearchResult` | [04 §10](04-features.md#10-検索フィルタ) |
| `search_index_status` | `connectionId` | `IndexStatus` | |
| `search_index_rebuild` | `connectionId`、`onEvent: Channel<IndexEvent>` | `JobId` | 全件走査 |
| `search_index_delete` | `connectionId` | — | インデックスの削除 |

### 3.7 メトリクス・コスト

| コマンド | 引数 | 戻り値 | 処理 |
|---|---|---|---|
| `metrics_storage` | `connectionId`、`force` | `StorageMetrics` | [04 §12.2](04-features.md#122-利用容量cloudwatch) |
| `cost_summary` | `connectionId` | `CostSummary \| null` | 保存済みの結果を返す（なければ `null`）。Cost Explorer には問い合わせない |
| `cost_refresh` | `connectionId` | `CostSummary` | Cost Explorer に問い合わせて保存する。ダッシュボードの「更新」「取得」からのみ呼ぶ（[04 §13.4](04-features.md#134-取得のタイミングと料金)） |
| `pricing_get` | `region` | `PriceTable` | [04 §13.3](04-features.md#133-単価price-list-api) |

### 3.8 設定・アプリ

| コマンド | 引数 | 戻り値 | 処理 |
|---|---|---|---|
| `settings_get` | — | `Settings` | |
| `settings_update` | `patch: Partial<Settings>` | `Settings` | 保存し、`settings://changed` を全ウィンドウに送る。ダウンロード先（`general.downloadDir`）は §3.9 のため受け取らず、無視する |
| `app_choose_download_dir` | — | `Settings` | Rust 側でフォルダ選択ダイアログを開き、選んだフォルダをダウンロード先に保存する |
| `app_set_theme` | `theme: 'auto' \| 'light' \| 'dark'` | — | 全ウィンドウの外観を切り替える（[02 §7.2](02-ui-foundation.md#72-ダークライトモードへの追従req-d04)） |
| `app_open_settings` | — | — | 設定ウィンドウを開く（開いていれば前面に出す） |
| `app_startup_info` | — | `{ session, dbRecreated, version }` | 起動時に一度だけ知らせること（SQLite の作り直しなど） |
| `app_notify` | `title`、`body` | — | ウィンドウが前面にないときだけ macOS の通知を出す（設定「完了時に通知する」に従う） |
| `menu_update_state` | `state: MenuState` | — | メニュー項目の有効・無効を更新する |
| `app_open_logs` | — | — | ログフォルダを Finder で開く |
| `app_clear_cache` | — | — | メトリクス・コストなどのキャッシュを削除する |
| `app_check_update` | — | `{ version, notes } \| null` | 更新の確認 |
| `app_install_update` | `onEvent: Channel<UpdateEvent>` | — | 更新のダウンロードと適用、再起動 |

### 3.9 ローカルパスの受け渡し

アップロード元とダウンロード先のローカルパスは、フロントエンドから文字列で受け取らない（既定のダウンロード先も、`settings_update` では変更できず、Rust 側のフォルダ選択 `app_choose_download_dir` でだけ変更する）。また、S3 のキーの階層（`..` など）で保存先の外に書き込まないよう、ダウンロードでは空・`.`・`..` の階層を含む項目を保存しない（[04 §5.3](04-features.md#53-フォルダ複数選択)）。Rust 側がファイル選択ダイアログ（`pick_upload_files`、`pick_download_dir`）とウィンドウのドラッグ＆ドロップイベントで得たパスを `SelectionRegistry` に保管し、フロントエンドには ID（`selectionId`）と表示用の名前・サイズだけを渡す。選択は 10 分で失効する。これにより、万一フロントエンドが不正なスクリプトに乗っ取られても、ユーザーが選んでいないファイルを読み出したり、任意の場所に書き込んだりできない（[07 §5](07-security.md#5-tauri-のセキュリティ設定)）。

## 4. イベントとチャネル

### 4.1 チャネル（呼び出しごと）

| チャネル | 渡すコマンド | 内容 |
|---|---|---|
| `Channel<TransferEvent>` | `transfer_subscribe` | 全転送ジョブの状態更新 |
| `Channel<BatchEvent>` | `objects_delete`、`objects_move`、`object_rename`、`objects_change_storage_class` | 進捗と結果 |
| `Channel<IndexEvent>` | `search_index_rebuild` | 走査済み件数と完了 |
| `Channel<UpdateEvent>` | `app_install_update` | 更新のダウンロード進捗 |

### 4.2 グローバルイベント（Rust → 全ウィンドウ）

| イベント | ペイロード | 用途 |
|---|---|---|
| `session://changed` | `UserSession \| null` | サインイン状態の変化 |
| `settings://changed` | `Settings` | 設定ウィンドウでの変更をメインウィンドウに反映 |
| `connections://changed` | — | 接続一覧の再取得 |
| `menu://action` | `{ id: string }` | アプリのメニュー・メニューバー常駐の項目が選ばれた |
| `dragdrop://enter`／`dragdrop://over`／`dragdrop://leave` | `{ position: { x, y }, names: string[] }` | Finder からのドラッグ中の表示 |
| `dragdrop://drop` | `{ selectionId, position, names }` | ドロップされた（パスは `SelectionRegistry` に保管済み） |
| `restore://completed` | `{ connectionId, key }` | アーカイブの取り出し完了 |
| `index://updated` | `{ connectionId, status: IndexStatus }` | 背景でのインデックス更新の完了 |
| `update://available` | `{ version, notes }` | 自動確認で新しいバージョンが見つかった（[08 §7](08-cicd.md#7-自動更新)） |

## 5. エラーコード

| コード | 意味 | 表示する文言（例） | 再試行 | UI の扱い |
|---|---|---|---|---|
| `NETWORK` | 通信できない | 接続できませんでした。ネットワークを確認してください | ○ | 再試行ボタン |
| `TIMEOUT` | 応答がない | 応答がありませんでした | ○ | 再試行ボタン |
| `SLOW_DOWN` | リクエストが多すぎる | しばらくしてからもう一度お試しください | ○ | 自動リトライ後に表示 |
| `AUTH_REQUIRED` | サインインが必要 | もう一度サインインしてください | × | SCR-01 |
| `AUTH_CANCELED` | サインインの中断 | サインインがキャンセルされました | × | — |
| `AUTH_NOT_ALLOWED` | 許可されていないアカウント | このアカウントは利用が許可されていません | × | — |
| `CREDENTIALS_INVALID` | アクセスキーが無効 | 認証に失敗しました。アクセスキーとシークレットキーを確認してください | × | DLG-06 |
| `CREDENTIALS_EXPIRED` | 一時認証情報を取り直せない | 認証情報の有効期限が切れました | × | DLG-06 |
| `ROLE_ASSUME_DENIED` | ロールを引き受けられない | ロールを引き受けられませんでした。信頼ポリシーと権限を確認してください | × | DLG-06 |
| `BUCKET_NOT_FOUND` | バケットがない | バケットが見つかりません | × | 接続の編集 |
| `BUCKET_ACCESS_DENIED` | バケットの権限がない | このバケットへのアクセス権がありません | × | 接続の編集 |
| `ACCESS_DENIED` | 操作の権限がない | この操作を行う権限がありません（{IAM アクション}） | × | トースト |
| `NOT_FOUND` | 対象がない | 項目が見つかりません。ほかの操作で削除された可能性があります | × | 一覧を更新 |
| `ALREADY_EXISTS` | 同名の項目がある | 同じ名前の項目があります | × | 入力欄に表示 |
| `INVALID_NAME` | 名前が不正（ダウンロードでは、保存先の外を指す名前） | この名前は使用できません | × | 入力欄に表示（ダウンロードでは失敗の一覧） |
| `INVALID_OBJECT_STATE` | 取り出していないアーカイブ | 取り出しが必要です | × | DLG-07 |
| `RESTORE_IN_PROGRESS` | 取り出し中 | 取り出し中です。完了したら通知します | × | — |
| `EXPEDITED_UNAVAILABLE` | 迅速な取り出しが使えない | 迅速な取り出しは現在利用できません。標準を選んでください | × | DLG-07 |
| `PRECONDITION_FAILED` | 処理中に対象が変更された | 項目が変更されたため中止しました | ○ | 自動で 1 回やり直し |
| `FILE_CHANGED` | 送信中にローカルファイルが変更された | 送信中にファイルが変更されました | ○ | 失敗一覧 |
| `LOCAL_IO` | ローカルの読み書きエラー | ファイルを読み書きできませんでした（{パス}） | × | 失敗一覧 |
| `DISK_FULL` | 空き容量不足 | ディスクの空き容量が足りません | × | トースト |
| `MFA_DELETE_REQUIRED` | MFA Delete が有効 | MFA Delete が有効なため、アプリからはバージョンを削除できません | × | トースト |
| `OBJECT_LOCKED` | Object Lock による保護 | このバージョンは保護期間中のため削除できません | × | トースト |
| `COST_UNAVAILABLE` | Cost Explorer が使えない（未有効化・権限なし・データなし） | コスト情報を取得できません（{理由}） | × | カード内に表示 |
| `METRICS_UNAVAILABLE` | CloudWatch のメトリクスがない・権限なし | —（インデックスからの集計に切り替える） | × | 注記 |
| `CANCELED` | ユーザーによるキャンセル | —（エラーとして表示しない） | × | — |
| `INTERNAL` | 想定外のエラー | 予期しないエラーが発生しました | × | 「詳細」でログの場所を示す |

AWS のエラーコードとの対応:

| AWS のエラー | コード |
|---|---|
| `InvalidAccessKeyId`、`SignatureDoesNotMatch`、`InvalidClientTokenId` | `CREDENTIALS_INVALID` |
| `ExpiredToken`、`ExpiredTokenException`（再取得に失敗した場合） | `CREDENTIALS_EXPIRED` |
| `AccessDenied`、`AccessDeniedException` | 文脈に応じて `BUCKET_ACCESS_DENIED`／`ROLE_ASSUME_DENIED`／`ACCESS_DENIED`／`COST_UNAVAILABLE` |
| `NoSuchBucket` | `BUCKET_NOT_FOUND` |
| `NoSuchKey`、`NoSuchVersion`、HEAD の 404 | `NOT_FOUND` |
| `InvalidObjectState` | `INVALID_OBJECT_STATE` |
| `RestoreAlreadyInProgress` | `RESTORE_IN_PROGRESS` |
| `GlacierExpeditedRetrievalNotAvailable` | `EXPEDITED_UNAVAILABLE` |
| `PreconditionFailed`（412） | `PRECONDITION_FAILED` |
| `SlowDown`、`503` | `SLOW_DOWN` |
| `RequestTimeout`、タイムアウト | `TIMEOUT` |
| 接続失敗（DispatchFailure） | `NETWORK` |
| `DataUnavailableException`（Cost Explorer） | `COST_UNAVAILABLE` |

`detail` には AWS のエラーコード、リクエスト ID、HTTP ステータスを入れる（秘密情報は含めない）。

## 6. 型の共有と呼び出し方

### 6.1 型の生成

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)] // 出力先は .cargo/config.toml の TS_RS_EXPORT_DIR（src/lib/ipc/bindings）
pub struct ObjectVersion {
    pub version_id: String,
    pub is_latest: bool,
    pub is_delete_marker: bool,
    pub last_modified: String,
    #[ts(type = "number | null")]
    pub size: Option<u64>,
    pub storage_class: Option<StorageClass>,
    pub etag: Option<String>,
}
```

- 型は `cargo test` の実行時に `src/lib/ipc/bindings/` へ書き出される。生成物はリポジトリに含め、CI で差分がないことを確認する（[08 §3](08-cicd.md#3-ciyml)）。

### 6.2 フロントエンドの呼び出し

```ts
// src/lib/ipc/index.ts（抜粋）
import { Channel, invoke } from '@tauri-apps/api/core';
import type { AppError, BatchEvent, ListPage, ObjectDetail, Target } from './bindings';

// erasableSyntaxOnly（01 §8.3）のため、コンストラクタ引数によるプロパティ宣言は使わない
export class IpcError extends Error {
  readonly error: AppError;

  constructor(error: AppError) {
    super(error.message);
    this.error = error;
  }
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    throw new IpcError(e as AppError);
  }
}

export const objects = {
  listPage: (connectionId: string, prefix: string, token: string | null, opts: ListOptions) =>
    call<ListPage>('objects_list_page', { connectionId, prefix, token, opts }),

  head: (connectionId: string, key: string, versionId?: string) =>
    call<ObjectDetail>('object_head', { connectionId, key, versionId }),

  delete: (connectionId: string, targets: Target[], allVersions: boolean, onEvent: (e: BatchEvent) => void) => {
    const channel = new Channel<BatchEvent>();
    channel.onmessage = onEvent;
    return call<string>('objects_delete', { connectionId, targets, allVersions, onEvent: channel });
  },
};
```

### 6.3 Rust のコマンド

```rust
#[tauri::command]
pub async fn objects_list_page(
    state: State<'_, AppState>,
    connection_id: ConnectionId,
    prefix: String,
    token: Option<String>,
    opts: ListOptions,
) -> Result<ListPage, AppError> {
    state
        .core
        .objects()
        .list_page(&connection_id, &prefix, token, opts)
        .await
        .map_err(AppError::from)
}
```

コマンド層では、引数の検証（プレフィックスの形式、名前の検証など）とエラー変換以外の処理を書かない。

## 7. 並行処理とキャンセル

| 項目 | 方針 |
|---|---|
| 短い処理 | コマンドの中で `await` して結果を返す |
| 長い処理 | ジョブを登録して即座に `JobId` を返し、進捗と結果はチャネルで送る |
| キャンセル | `JobRegistry` がジョブごとの `CancellationToken` を持つ。ジョブの処理は `tokio::select!` でトークンを監視する |
| 並列数 | 転送は全体のセマフォ（ファイル数）とファイルごとのセマフォ（パート数）。一括操作はジョブごとに `buffer_unordered(8)` |
| 一括キャンセル | サインアウト時は全ジョブ、接続の削除時はその接続のジョブをキャンセルする |
| ロック | `await` をまたいで同期ロックを保持しない。共有状態は `tokio::sync::RwLock` か、短い区間の `std::sync::Mutex` に限る |

## 8. Tauri の権限（capabilities）

- `build.rs` の `AppManifest` にアプリのコマンドを列挙し、ウィンドウごとの capability で許可するコマンドを明示する。
- メインウィンドウ（`capabilities/main.json`）は全コマンドと、ウィンドウ操作（ドラッグ開始、表示、フォーカス、フォーカスの確認）、イベント、ログ、クリップボードへの書き込みを許可する。通知と外観の切り替えは、プラグインの権限を与えずにコマンド（`app_notify`、`app_set_theme`）で Rust 側から行う。
- 設定ウィンドウ（`capabilities/settings.json`）は `settings_*`、`connection_*`、`credential_*`、`search_index_*`、`app_*` のみを許可する。
- ファイル選択・Finder での表示・URL を開く処理は Rust 側で行うため、フロントエンドに dialog／opener／fs の権限は与えない。

詳細は [07 §5](07-security.md#5-tauri-のセキュリティ設定) を参照する。

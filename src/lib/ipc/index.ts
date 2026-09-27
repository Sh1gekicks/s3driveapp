// IPC クライアント（05 §6.2）。`invoke` を呼んでよいのはこのモジュールだけとする（01 §5.1）。

import { Channel, invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AppError,
  Appearance,
  BatchEvent,
  BatchResult,
  BucketInfo,
  Connection,
  ConnectionInput,
  ConnectionPatch,
  ConnectionTestResult,
  CostSummary,
  CredentialSummary,
  Decisions,
  DownloadDestination,
  DragPayload,
  DropPayload,
  Entry,
  ErrorCode,
  FolderSummary,
  IndexEvent,
  IndexStatus,
  IndexUpdated,
  ListOptions,
  ListPage,
  Location,
  MenuAction,
  MenuState,
  ObjectDetail,
  ObjectVersion,
  PriceTable,
  RemoteConflict,
  RestoreCompleted,
  RestoreTier,
  SearchQuery,
  SearchResult,
  Selection,
  Settings,
  StartupInfo,
  StorageClass,
  StorageMetrics,
  Target,
  TransferEvent,
  TransferJob,
  UpdateEvent,
  UpdateInfo,
  UploadPlan,
  UserSession,
  VersionPage,
} from './types';

export type * from './types';

/** IPC のエラー。`erasableSyntaxOnly` のため、コンストラクタ引数によるプロパティ宣言は使わない。 */
export class IpcError extends Error {
  readonly error: AppError;

  constructor(error: AppError) {
    super(error.message);
    this.name = 'IpcError';
    this.error = error;
  }

  get code(): ErrorCode {
    return this.error.code;
  }
}

function isAppError(value: unknown): value is AppError {
  return typeof value === 'object' && value !== null && 'code' in value && 'message' in value;
}

/** 任意の失敗を AppError の形にする。 */
export function toAppError(e: unknown): AppError {
  if (e instanceof IpcError) return e.error;
  if (isAppError(e)) return e;
  return {
    code: 'INTERNAL',
    message: '予期しないエラーが発生しました',
    detail: e instanceof Error ? e.message : String(e),
    retryable: false,
  };
}

export function errorCode(e: unknown): ErrorCode {
  return toAppError(e).code;
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    throw new IpcError(toAppError(e));
  }
}

function channel<T>(onEvent: (e: T) => void): Channel<T> {
  const ch = new Channel<T>();
  ch.onmessage = onEvent;
  return ch;
}

export const auth = {
  getSession: () => call<UserSession | null>('auth_get_session'),
  restore: () => call<UserSession | null>('auth_restore'),
  signIn: () => call<UserSession>('auth_sign_in'),
  cancelSignIn: () => call<void>('auth_cancel_sign_in'),
  signOut: () => call<void>('auth_sign_out'),
};

export const connections = {
  list: () => call<Connection[]>('connection_list'),
  test: (input: ConnectionInput) => call<ConnectionTestResult>('connection_test', { input }),
  create: (input: ConnectionInput) => call<Connection>('connection_create', { input }),
  update: (id: string, input: ConnectionInput) => call<Connection>('connection_update', { id, input }),
  patch: (id: string, patch: ConnectionPatch) => call<Connection>('connection_patch', { id, patch }),
  remove: (id: string) => call<void>('connection_delete', { id }),
  reorder: (ids: string[]) => call<void>('connection_reorder', { ids }),
  lastLocation: () => call<Location | null>('connection_last_location'),
  setLocation: (location: Location) => call<void>('connection_set_location', { location }),
  credentials: () => call<CredentialSummary[]>('credential_list'),
  updateCredential: (credentialId: string, accessKeyId: string, secretAccessKey: string) =>
    call<void>('credential_update', { credentialId, accessKeyId, secretAccessKey }),
  bucketInfo: (connectionId: string, force = false) =>
    call<BucketInfo>('bucket_get_info', { connectionId, force }),
};

export const objects = {
  listPage: (connectionId: string, prefix: string, token: string | null, opts: ListOptions) =>
    call<ListPage>('objects_list_page', { connectionId, prefix, token, opts }),
  head: (connectionId: string, key: string, versionId?: string) =>
    call<ObjectDetail>('object_head', { connectionId, key, versionId }),
  createFolder: (connectionId: string, prefix: string, name: string) =>
    call<Entry>('folder_create', { connectionId, prefix, name }),
  folderSummary: (connectionId: string, prefix: string) =>
    call<FolderSummary>('folder_summary', { connectionId, prefix }),
  folderChildren: (connectionId: string, prefix: string) =>
    call<Entry[]>('folder_children', { connectionId, prefix }),
  findConflicts: (connectionId: string, targets: Target[], destPrefix: string) =>
    call<RemoteConflict[]>('objects_find_conflicts', { connectionId, targets, destPrefix }),
  delete: (connectionId: string, targets: Target[], allVersions: boolean, onEvent: (e: BatchEvent) => void) =>
    call<string>('objects_delete', { connectionId, targets, allVersions, onEvent: channel(onEvent) }),
  move: (
    connectionId: string,
    targets: Target[],
    destPrefix: string,
    decisions: Decisions,
    onEvent: (e: BatchEvent) => void,
  ) =>
    call<string>('objects_move', { connectionId, targets, destPrefix, decisions, onEvent: channel(onEvent) }),
  rename: (connectionId: string, target: Target, newName: string, onEvent: (e: BatchEvent) => void) =>
    call<string>('object_rename', { connectionId, target, newName, onEvent: channel(onEvent) }),
  changeStorageClass: (
    connectionId: string,
    targets: Target[],
    storageClass: StorageClass,
    onEvent: (e: BatchEvent) => void,
  ) =>
    call<string>('objects_change_storage_class', {
      connectionId,
      targets,
      storageClass,
      onEvent: channel(onEvent),
    }),
  requestRestore: (connectionId: string, targets: Target[], tier: RestoreTier, days?: number) =>
    call<BatchResult>('objects_request_restore', { connectionId, targets, tier, days }),
};

export const versions = {
  list: (connectionId: string, key: string, cursor?: string | null) =>
    call<VersionPage>('versions_list', { connectionId, key, cursor: cursor ?? null }),
  restore: (connectionId: string, key: string, versionId: string) =>
    call<ObjectVersion>('version_restore', { connectionId, key, versionId }),
  remove: (connectionId: string, key: string, versionId: string) =>
    call<void>('version_delete', { connectionId, key, versionId }),
  undelete: (connectionId: string, keys: string[]) =>
    call<BatchResult>('deleted_restore', { connectionId, keys }),
};

export const transfers = {
  subscribe: (onEvent: (e: TransferEvent) => void) =>
    call<TransferJob[]>('transfer_subscribe', { onEvent: channel(onEvent) }),
  pickUploadFiles: (directories: boolean) => call<Selection | null>('pick_upload_files', { directories }),
  prepareUpload: (connectionId: string, prefix: string, selectionId: string) =>
    call<UploadPlan>('upload_prepare', { connectionId, prefix, selectionId }),
  startUpload: (planId: string, decisions: Decisions) => call<string>('upload_start', { planId, decisions }),
  pickDownloadDir: () => call<Selection | null>('pick_download_dir'),
  startDownload: (connectionId: string, targets: Target[], destination: DownloadDestination) =>
    call<string>('download_start', { connectionId, targets, destination }),
  reveal: (jobId: string) => call<void>('download_reveal', { jobId }),
  cancel: (jobId: string) => call<void>('job_cancel', { jobId }),
  retry: (jobId: string) => call<string>('transfer_retry', { jobId }),
  clearFinished: () => call<void>('transfer_clear_finished'),
};

export const search = {
  query: (connectionId: string, query: SearchQuery) =>
    call<SearchResult>('search_query', { connectionId, query }),
  status: (connectionId: string) => call<IndexStatus>('search_index_status', { connectionId }),
  rebuild: (connectionId: string, onEvent: (e: IndexEvent) => void) =>
    call<string>('search_index_rebuild', { connectionId, onEvent: channel(onEvent) }),
  remove: (connectionId: string) => call<void>('search_index_delete', { connectionId }),
};

export const metrics = {
  storage: (connectionId: string, force = false) =>
    call<StorageMetrics>('metrics_storage', { connectionId, force }),
  /** 保存済みの結果（Cost Explorer には問い合わせない）。 */
  cost: (connectionId: string) => call<CostSummary | null>('cost_summary', { connectionId }),
  /** Cost Explorer に問い合わせる。「更新」「取得」からのみ呼ぶ（04 §13.4）。 */
  refreshCost: (connectionId: string) => call<CostSummary>('cost_refresh', { connectionId }),
  pricing: (region: string) => call<PriceTable>('pricing_get', { region }),
};

/** 設定の部分更新。 */
export type SettingsPatch = { [K in keyof Settings]?: Partial<Settings[K]> };

export const app = {
  settings: () => call<Settings>('settings_get'),
  updateSettings: (patch: SettingsPatch) => call<Settings>('settings_update', { patch }),
  setTheme: (theme: Appearance) => call<void>('app_set_theme', { theme }),
  chooseDownloadDir: () => call<Settings>('app_choose_download_dir'),
  openSettings: () => call<void>('app_open_settings'),
  startupInfo: () => call<StartupInfo>('app_startup_info'),
  updateMenuState: (state: MenuState) => call<void>('menu_update_state', { state }),
  openLogs: () => call<void>('app_open_logs'),
  clearCache: () => call<void>('app_clear_cache'),
  checkUpdate: () => call<UpdateInfo | null>('app_check_update'),
  installUpdate: (onEvent: (e: UpdateEvent) => void) =>
    call<void>('app_install_update', { onEvent: channel(onEvent) }),
  notify: (title: string, body: string) => call<void>('app_notify', { title, body }),
};

/** グローバルイベント（05 §4.2）。 */
export const events = {
  onSession: (cb: (s: UserSession | null) => void) => on<UserSession | null>('session://changed', cb),
  onSettings: (cb: (s: Settings) => void) => on<Settings>('settings://changed', cb),
  onConnections: (cb: () => void) => on<null>('connections://changed', () => cb()),
  onMenu: (cb: (a: MenuAction) => void) => on<MenuAction>('menu://action', cb),
  onDragEnter: (cb: (p: DragPayload) => void) => on<DragPayload>('dragdrop://enter', cb),
  onDragOver: (cb: (p: DragPayload) => void) => on<DragPayload>('dragdrop://over', cb),
  onDragLeave: (cb: (p: DragPayload) => void) => on<DragPayload>('dragdrop://leave', cb),
  onDrop: (cb: (p: DropPayload) => void) => on<DropPayload>('dragdrop://drop', cb),
  onRestoreCompleted: (cb: (p: RestoreCompleted) => void) => on<RestoreCompleted>('restore://completed', cb),
  onIndexUpdated: (cb: (p: IndexUpdated) => void) => on<IndexUpdated>('index://updated', cb),
  onUpdateAvailable: (cb: (p: UpdateInfo) => void) => on<UpdateInfo>('update://available', cb),
};

function on<T>(event: string, cb: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(event, (e) => cb(e.payload));
}

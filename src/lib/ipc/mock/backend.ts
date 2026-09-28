// IPC コマンドのメモリ実装（09 §2.4）。`pnpm dev:mock` と画面テストで、Tauri なしに画面を動かす。

import type { Channel } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';
import { extensionOf, fileKind } from '../../file-kind';
import { sortEntries } from '../../sort';
import type {
  AppError,
  BatchEvent,
  BatchResult,
  Connection,
  ConnectionInput,
  CostSummary,
  Decisions,
  Entry,
  ErrorCode,
  IndexStatus,
  ListOptions,
  ObjectVersion,
  RestoreState,
  SearchEntry,
  SearchQuery,
  Selection,
  Settings,
  StorageClass,
  Target,
  TransferEvent,
  TransferJob,
  UploadPlan,
  UserSession,
} from '../types';
import { contentTypeOf, type MockBucket, type MockObject, NOW, PRICE, seed, versionId } from './data';

const SESSION: UserSession = {
  sub: 'mock-user',
  email: 'yuki.tanaka@gmail.com',
  name: '田中 優希',
  initial: '田',
};

const DEFAULT_SETTINGS: Settings = {
  general: { appearance: 'auto', downloadDir: null, showHidden: false, showMenuBarIcon: true },
  view: { mode: 'list', sort: { key: 'name', dir: 1 }, inspector: true },
  transfer: {
    maxFiles: 3,
    maxPartsPerFile: 4,
    multipartThresholdMb: 16,
    defaultStorageClass: 'STANDARD',
    normalizeNfc: true,
    ignore: ['.DS_Store'],
    notifyOnComplete: true,
  },
  cost: { useCostExplorer: true },
  search: { autoRefreshMinutes: 60 },
  advanced: { logLevel: 'info', autoCheckUpdate: true },
};

const REGION_NAMES: Record<string, [string, string]> = {
  'ap-northeast-1': ['アジアパシフィック (東京)', '東京'],
  'ap-northeast-3': ['アジアパシフィック (大阪)', '大阪'],
  'us-east-1': ['米国東部 (バージニア北部)', 'バージニア北部'],
};

function fail(code: ErrorCode, message: string): never {
  const err: AppError = { code, message, retryable: code === 'NETWORK' || code === 'TIMEOUT' };
  throw err;
}

const baseName = (key: string) => key.replace(/\/$/, '').split('/').pop() ?? key;
const parentOf = (key: string) => {
  const trimmed = key.replace(/\/$/, '');
  const i = trimmed.lastIndexOf('/');
  return i < 0 ? '' : trimmed.slice(0, i + 1);
};
const iso = (d: Date) => d.toISOString().replace(/\.\d{3}Z$/, 'Z');

interface MockSelection {
  items: { name: string; size: number; isDir: boolean }[];
}

interface MockPlan {
  connectionId: string;
  prefix: string;
  files: { key: string; size: number }[];
  conflicts: Set<string>;
}

interface MockJob {
  job: TransferJob;
  timer?: ReturnType<typeof setInterval>;
  onDone?: () => void;
}

export interface MockOptions {
  /** サインインしていない状態から始める（SCR-01 の確認用）。 */
  signedOut?: boolean;
  /** 接続がない状態から始める。 */
  noConnections?: boolean;
  /** 転送・一括操作の進み方（ミリ秒）。0 なら即座に完了する。 */
  tick?: number;
}

export class MockBackend {
  session: UserSession | null;
  settings: Settings = structuredClone(DEFAULT_SETTINGS);
  buckets: MockBucket[];
  objects: Map<string, Map<string, MockObject>>;
  costs = new Map<string, CostSummary>();
  selections = new Map<string, MockSelection>();
  plans = new Map<string, MockPlan>();
  jobs = new Map<string, MockJob>();
  transferChannel: Channel<TransferEvent> | null = null;
  lastLocation: { connectionId: string; prefix: string } | null = null;
  tick: number;
  calls: { cmd: string; args: Record<string, unknown> }[] = [];
  /** 次の呼び出しで失敗させるコマンド（画面の失敗表示のテスト用）。 */
  failures = new Map<string, AppError>();
  /** 検索インデックスを作成した接続（CloudWatch の容量がないときの集計元。04 §12.2）。 */
  indexed = new Set<string>();

  constructor(opts: MockOptions = {}) {
    const data = seed();
    this.buckets = opts.noConnections ? [] : data.buckets;
    this.objects = data.objects;
    this.session = opts.signedOut ? null : SESSION;
    this.tick = opts.tick ?? 180;
  }

  private bucket(connectionId: string): MockBucket {
    if (!this.session) fail('AUTH_REQUIRED', 'もう一度サインインしてください');
    const b = this.buckets.find((x) => x.id === connectionId);
    if (!b) fail('NOT_FOUND', '接続が見つかりません');
    return b;
  }

  private store(connectionId: string): Map<string, MockObject> {
    this.bucket(connectionId);
    let s = this.objects.get(connectionId);
    if (!s) {
      s = new Map();
      this.objects.set(connectionId, s);
    }
    return s;
  }

  private connection(b: MockBucket): Connection {
    const [label, short] = REGION_NAMES[b.region] ?? [b.region, b.region];
    return {
      id: b.id,
      bucket: b.name,
      region: b.region,
      regionLabel: label,
      regionShort: short,
      credentialId: 'cred-1',
      accessKeyIdMasked: 'AKIA************Q2LM',
      roleArn: b.id === 'conn-tokyo' ? 'arn:aws:iam::123456789012:role/S3DriveAccess' : null,
      externalId: null,
      useSourceIdentity: false,
      costTag: null,
      defaultStorageClass: null,
    };
  }

  private current(o: MockObject) {
    return o.versions[0];
  }

  /** 検索インデックスの状態（12 分前に作成済み）。 */
  private indexStatus(objectCount: number): IndexStatus {
    return {
      state: 'ready',
      objectCount,
      lastScanAt: iso(new Date(Date.now() - 12 * 60000)),
      sizeBytes: 4_200_000,
      progress: null,
    };
  }

  private isLive(o: MockObject) {
    const v = this.current(o);
    return v !== undefined && !v.deleteMarker;
  }

  private restoreState(o: MockObject): RestoreState {
    const v = this.current(o);
    if (!v) return { state: 'notArchived' };
    if (v.storageClass !== 'GLACIER' && v.storageClass !== 'DEEP_ARCHIVE') return { state: 'notArchived' };
    return o.restore ?? { state: 'archived' };
  }

  private fileEntry(o: MockObject, deleted = false): Entry {
    const live = o.versions.find((v) => !v.deleteMarker);
    const v = deleted ? live : this.current(o);
    return {
      type: 'file',
      key: o.key,
      name: baseName(o.key),
      size: v?.size ?? 0,
      lastModified: iso(this.current(o)?.date ?? NOW),
      etag: o.etag,
      storageClass: v?.storageClass ?? 'STANDARD',
      restore: this.restoreState(o),
      deleted,
    };
  }

  private listEntries(connectionId: string, prefix: string, opts: ListOptions): Entry[] {
    const store = this.store(connectionId);
    const folders = new Map<string, Entry>();
    const files: Entry[] = [];
    for (const o of store.values()) {
      if (!o.key.startsWith(prefix) || o.key === prefix) continue;
      const rest = o.key.slice(prefix.length);
      const slash = rest.indexOf('/');
      if (slash >= 0 && slash < rest.length - 1) {
        const folderKey = prefix + rest.slice(0, slash + 1);
        if (!folders.has(folderKey) && this.isLive(o)) {
          const marker = store.get(folderKey);
          folders.set(folderKey, {
            type: 'folder',
            key: folderKey,
            name: rest.slice(0, slash),
            lastModified: marker ? iso(this.current(marker)?.date ?? NOW) : null,
            deleted: false,
          });
        }
        continue;
      }
      if (rest.endsWith('/')) {
        if (this.isLive(o)) {
          folders.set(o.key, {
            type: 'folder',
            key: o.key,
            name: baseName(o.key),
            lastModified: iso(this.current(o)?.date ?? NOW),
            deleted: false,
          });
        } else if (opts.includeDeleted) {
          folders.set(o.key, {
            type: 'folder',
            key: o.key,
            name: baseName(o.key),
            lastModified: null,
            deleted: true,
          });
        }
        continue;
      }
      if (this.isLive(o)) files.push(this.fileEntry(o));
      else if (opts.includeDeleted && o.versions.some((v) => !v.deleteMarker))
        files.push(this.fileEntry(o, true));
    }
    let entries = [...folders.values(), ...files];
    if (!opts.showHidden) entries = entries.filter((e) => !e.name.startsWith('.'));
    return entries;
  }

  private expand(connectionId: string, targets: Target[]): MockObject[] {
    const store = this.store(connectionId);
    const out: MockObject[] = [];
    for (const t of targets) {
      if (t.isFolder) {
        for (const o of store.values()) if (o.key.startsWith(t.key)) out.push(o);
      } else {
        const o = store.get(t.key);
        if (o) out.push(o);
      }
    }
    return out;
  }

  /** 一括操作をジョブとして進め、チャネルに進捗と結果を送る。 */
  private runBatch(onEvent: Channel<BatchEvent>, total: number, work: () => BatchResult): string {
    const jobId = crypto.randomUUID();
    const finish = () => {
      const result = work();
      onEvent.onmessage({ event: 'progress', data: { jobId, done: total, total } });
      onEvent.onmessage({ event: 'finished', data: { jobId, result } });
    };
    onEvent.onmessage({ event: 'progress', data: { jobId, done: 0, total } });
    if (this.tick === 0) queueMicrotask(finish);
    else setTimeout(finish, this.tick * 2);
    return jobId;
  }

  private emitJob(job: TransferJob) {
    this.transferChannel?.onmessage({ event: 'jobUpdated', data: { ...job } });
  }

  /** 転送を少しずつ進める。 */
  private runTransfer(job: TransferJob, onDone: () => void): string {
    const entry: MockJob = { job, onDone };
    this.jobs.set(job.jobId, entry);
    this.emitJob(job);
    const step = () => {
      if (job.status === 'canceled') {
        clearInterval(entry.timer);
        return;
      }
      job.status = 'running';
      // 1 回に全体の 16% ずつ進める（見た目を安定させるため乱数は使わない）
      const chunk = Math.max(1, Math.round(job.totalBytes * 0.16));
      job.doneBytes = Math.min(job.totalBytes, job.doneBytes + chunk);
      job.bytesPerSec = chunk * (1000 / Math.max(1, this.tick));
      job.etaSec = job.bytesPerSec > 0 ? Math.ceil((job.totalBytes - job.doneBytes) / job.bytesPerSec) : null;
      if (job.doneBytes >= job.totalBytes) {
        clearInterval(entry.timer);
        job.status = 'succeeded';
        job.doneFiles = job.totalFiles;
        job.currentName = null;
        job.etaSec = null;
        onDone();
      }
      this.emitJob(job);
    };
    if (this.tick === 0) {
      queueMicrotask(() => {
        job.doneBytes = job.totalBytes;
        job.status = 'succeeded';
        job.doneFiles = job.totalFiles;
        onDone();
        this.emitJob(job);
      });
    } else {
      entry.timer = setInterval(step, this.tick);
    }
    return job.jobId;
  }

  /** 1 つのコマンドを処理する。 */
  /** 次の呼び出しで応答を保留するコマンド（読み込み中の表示のテスト用）。 */
  holds = new Map<string, Promise<void>>();

  /** `cmd` の次の呼び出しを `code` で失敗させる。 */
  failNext(cmd: string, code: ErrorCode, message: string): void {
    this.failures.set(cmd, { code, message, retryable: false });
  }

  /** `cmd` の次の呼び出しの応答を、戻り値の関数を呼ぶまで保留する。 */
  holdNext(cmd: string): () => void {
    let release = () => {};
    this.holds.set(
      cmd,
      new Promise<void>((resolve) => {
        release = resolve;
      }),
    );
    return () => release();
  }

  async handle(cmd: string, args: Record<string, unknown> = {}): Promise<unknown> {
    this.calls.push({ cmd, args });
    const failure = this.failures.get(cmd);
    if (failure) {
      this.failures.delete(cmd);
      throw failure;
    }
    const hold = this.holds.get(cmd);
    if (hold) {
      this.holds.delete(cmd);
      await hold;
    }
    // biome-ignore lint/suspicious/noExplicitAny: モックの引数の型はコマンドごとに異なるため、各分岐で扱う
    const a = args as Record<string, any>;
    switch (cmd) {
      // 認証
      case 'auth_get_session':
        return this.session;
      case 'auth_restore':
        return this.session;
      case 'auth_sign_in':
        await new Promise((r) => setTimeout(r, Math.min(this.tick * 4, 700)));
        this.session = SESSION;
        void emit('session://changed', this.session);
        return this.session;
      case 'auth_cancel_sign_in':
        return null;
      case 'auth_sign_out':
        this.session = null;
        void emit('session://changed', null);
        return null;

      // 接続
      case 'connection_list':
        if (!this.session) fail('AUTH_REQUIRED', 'もう一度サインインしてください');
        return this.buckets.map((b) => this.connection(b));
      case 'connection_test':
      case 'connection_create':
      case 'connection_update': {
        const input = a.input as ConnectionInput;
        if (input.credential.kind === 'new' && !input.credential.accessKeyId.startsWith('AKIA')) {
          fail('CREDENTIALS_INVALID', '認証に失敗しました。アクセスキーとシークレットキーを確認してください');
        }
        if (input.bucket === 'missing-bucket') fail('BUCKET_NOT_FOUND', 'バケットが見つかりません');
        if (input.bucket === 'denied-bucket')
          fail('BUCKET_ACCESS_DENIED', 'このバケットへのアクセス権がありません');
        await new Promise((r) => setTimeout(r, Math.min(this.tick * 3, 600)));
        const region = input.bucket.includes('osaka') ? 'ap-northeast-3' : input.region;
        if (cmd === 'connection_test') {
          return {
            accountId: '123456789012',
            callerArn: 'arn:aws:iam::123456789012:user/s3drive-user',
            region,
            regionCorrected: region !== input.region,
            versioning: 'enabled',
          };
        }
        const existing = cmd === 'connection_update' ? this.buckets.find((b) => b.id === a.id) : undefined;
        const b: MockBucket = existing ?? {
          id: `conn-${crypto.randomUUID().slice(0, 8)}`,
          name: input.bucket,
          region,
          versioning: 'enabled',
          encryption: 'SSE-S3 (AES-256)',
          objectCount: 0,
          usageGb: {},
          cost: { requests: 0, transfer: 0, retrieval: 0, prevMonth: 0, dailyBase: 0 },
        };
        b.name = input.bucket;
        b.region = region;
        if (!existing) {
          this.buckets.push(b);
          this.objects.set(b.id, new Map());
        }
        void emit('connections://changed', null);
        return this.connection(b);
      }
      case 'connection_patch': {
        const b = this.bucket(a.id);
        void emit('connections://changed', null);
        return { ...this.connection(b), ...(a.patch as object) };
      }
      case 'connection_delete':
        this.buckets = this.buckets.filter((b) => b.id !== a.id);
        void emit('connections://changed', null);
        return null;
      case 'connection_reorder':
        return null;
      case 'connection_last_location':
        return this.lastLocation;
      case 'connection_set_location':
        this.lastLocation = a.location as never;
        return null;
      case 'credential_list':
        return [
          {
            id: 'cred-1',
            accessKeyIdMasked: 'AKIA************Q2LM',
            usedBy: this.buckets.map((b) => b.name),
          },
        ];
      case 'credential_update':
        return null;
      case 'bucket_get_info': {
        const b = this.bucket(a.connectionId);
        return {
          bucket: b.name,
          region: b.region,
          versioning: b.versioning,
          encryption: b.encryption,
          regionCorrected: false,
        };
      }

      // オブジェクト
      case 'objects_list_page': {
        const entries = this.listEntries(a.connectionId, a.prefix, a.opts as ListOptions);
        return { entries, nextToken: null };
      }
      case 'object_head': {
        const store = this.store(a.connectionId);
        const o = store.get(a.key);
        if (!o) fail('NOT_FOUND', '項目が見つかりません。ほかの操作で削除された可能性があります');
        const v = a.versionId ? o.versions.find((x) => x.id === a.versionId) : this.current(o);
        const oldest = [...o.versions].reverse().find((x) => !x.deleteMarker);
        const versioned = this.bucket(a.connectionId).versioning !== 'disabled';
        return {
          key: o.key,
          versionId: versioned ? (v?.id ?? null) : null,
          size: v?.size ?? 0,
          contentType: o.contentType,
          lastModified: iso(v?.date ?? NOW),
          created:
            versioned && oldest
              ? { at: iso(oldest.date), source: 'oldestVersion' }
              : { at: iso(v?.date ?? NOW), source: 'lastModified' },
          etag: o.etag,
          storageClass: v?.storageClass ?? 'STANDARD',
          encryption: 'SSE-S3 (AES-256)',
          kmsKeyId: null,
          checksums: { CRC32: 'rOxIjA==' },
          userMetadata: o.metadata,
          restore: this.restoreState(o),
        };
      }
      case 'folder_create': {
        const store = this.store(a.connectionId);
        const key = `${a.prefix}${String(a.name).trim()}/`;
        if ([...store.keys()].some((k) => k.startsWith(key))) {
          fail('ALREADY_EXISTS', '同じ名前のフォルダがあります');
        }
        store.set(key, {
          key,
          contentType: 'application/x-directory',
          etag: '',
          metadata: {},
          versions: [
            { id: versionId(), date: new Date(), size: 0, deleteMarker: false, storageClass: 'STANDARD' },
          ],
        });
        return {
          type: 'folder',
          key,
          name: String(a.name).trim(),
          lastModified: iso(new Date()),
          deleted: false,
        };
      }
      case 'folder_summary': {
        const entries = this.listEntries(a.connectionId, a.prefix, {
          showHidden: true,
          includeDeleted: false,
        });
        const store = this.store(a.connectionId);
        let total = 0;
        for (const o of store.values()) {
          if (o.key.startsWith(a.prefix) && this.isLive(o)) total += this.current(o)?.size ?? 0;
        }
        return { itemCount: entries.length, totalBytes: total, truncated: false };
      }
      case 'folder_children':
        return this.listEntries(a.connectionId, a.prefix, { showHidden: true, includeDeleted: false }).filter(
          (e) => e.type === 'folder',
        );
      case 'objects_find_conflicts': {
        const store = this.store(a.connectionId);
        const out = [];
        for (const t of a.targets as Target[]) {
          const dest = `${a.destPrefix}${baseName(t.key)}${t.isFolder ? '/' : ''}`;
          const hit = t.isFolder
            ? [...store.values()].find((o) => o.key.startsWith(dest) && this.isLive(o))
            : store.get(dest);
          if (hit && this.isLive(hit)) {
            out.push({
              key: dest,
              remoteSize: this.current(hit)?.size ?? 0,
              remoteModified: iso(this.current(hit)?.date ?? NOW),
            });
          }
        }
        return out;
      }
      case 'objects_delete': {
        const b = this.bucket(a.connectionId);
        const store = this.store(a.connectionId);
        const items = this.expand(a.connectionId, a.targets as Target[]);
        return this.runBatch(a.onEvent, items.length, () => {
          for (const o of items) {
            if (a.allVersions || b.versioning === 'disabled') store.delete(o.key);
            else
              o.versions.unshift({
                id: versionId(),
                date: new Date(),
                size: null,
                deleteMarker: true,
                storageClass: 'STANDARD',
              });
          }
          return { succeeded: items.length, skipped: [], failed: [] };
        });
      }
      case 'objects_move':
      case 'object_rename': {
        const store = this.store(a.connectionId);
        const targets = cmd === 'object_rename' ? [a.target as Target] : (a.targets as Target[]);
        const items = this.expand(a.connectionId, targets);
        const decisions = (a.decisions ?? {}) as Decisions;
        return this.runBatch(a.onEvent, items.length, () => {
          const result: BatchResult = { succeeded: 0, skipped: [], failed: [] };
          for (const t of targets) {
            const parent = parentOf(t.key);
            const newRoot =
              cmd === 'object_rename'
                ? `${parent}${String(a.newName)}${t.isFolder ? '/' : ''}`
                : `${a.destPrefix}${baseName(t.key)}${t.isFolder ? '/' : ''}`;
            const decision =
              'all' in decisions ? decisions.all : (decisions as Record<string, string>)[newRoot];
            if (decision === 'skip' && store.has(newRoot)) {
              result.skipped.push({ key: t.key, reason: '同じ名前の項目があるためスキップしました' });
              continue;
            }
            for (const o of [...store.values()]) {
              if (o.key !== t.key && !(t.isFolder && o.key.startsWith(t.key))) continue;
              if (this.restoreState(o).state === 'archived') {
                result.skipped.push({
                  key: o.key,
                  reason: '取り出されていないアーカイブのため移動できません',
                });
                continue;
              }
              store.delete(o.key);
              const key = newRoot + o.key.slice(t.key.length);
              store.set(key, { ...o, key });
              result.succeeded++;
            }
          }
          return result;
        });
      }
      case 'objects_change_storage_class': {
        const items = this.expand(a.connectionId, a.targets as Target[]).filter((o) => !o.key.endsWith('/'));
        return this.runBatch(a.onEvent, items.length, () => {
          const result: BatchResult = { succeeded: 0, skipped: [], failed: [] };
          for (const o of items) {
            const v = this.current(o);
            if (!v) continue;
            if (v.storageClass === a.storageClass) {
              result.skipped.push({ key: o.key, reason: 'すでに同じストレージクラスです' });
            } else if (this.restoreState(o).state === 'archived') {
              result.skipped.push({ key: o.key, reason: '取り出しが必要なため変更できません' });
            } else {
              o.versions.unshift({
                ...v,
                id: versionId(),
                date: new Date(),
                storageClass: a.storageClass as StorageClass,
              });
              result.succeeded++;
            }
          }
          return result;
        });
      }
      case 'objects_request_restore': {
        const items = this.expand(a.connectionId, a.targets as Target[]);
        for (const o of items) o.restore = { state: 'inProgress' };
        if (this.tick > 0) {
          setTimeout(() => {
            for (const o of items)
              o.restore = { state: 'restored', expiry: iso(new Date(Date.now() + 7 * 864e5)) };
            for (const o of items)
              void emit('restore://completed', { connectionId: a.connectionId, key: o.key });
          }, this.tick * 20);
        }
        return { succeeded: items.length, skipped: [], failed: [] };
      }

      // バージョン
      case 'versions_list': {
        const o = this.store(a.connectionId).get(a.key);
        if (!o || this.bucket(a.connectionId).versioning === 'disabled')
          return { versions: [], nextCursor: null };
        const versions: ObjectVersion[] = o.versions.map((v, i) => ({
          versionId: v.id,
          isLatest: i === 0,
          isDeleteMarker: v.deleteMarker,
          lastModified: iso(v.date),
          size: v.size,
          storageClass: v.deleteMarker ? null : v.storageClass,
          etag: v.deleteMarker ? null : o.etag,
        }));
        return { versions, nextCursor: null };
      }
      case 'version_restore': {
        const o = this.store(a.connectionId).get(a.key);
        const v = o?.versions.find((x) => x.id === a.versionId);
        if (!o || !v) fail('NOT_FOUND', '項目が見つかりません。ほかの操作で削除された可能性があります');
        const restored = { ...v, id: versionId(), date: new Date() };
        o.versions.unshift(restored);
        return {
          versionId: restored.id,
          isLatest: true,
          isDeleteMarker: false,
          lastModified: iso(restored.date),
          size: restored.size,
          storageClass: restored.storageClass,
          etag: o.etag,
        };
      }
      case 'version_delete': {
        const store = this.store(a.connectionId);
        const o = store.get(a.key);
        if (o) {
          o.versions = o.versions.filter((v) => v.id !== a.versionId);
          if (o.versions.length === 0) store.delete(a.key);
        }
        return null;
      }
      case 'deleted_restore': {
        const store = this.store(a.connectionId);
        let n = 0;
        for (const key of a.keys as string[]) {
          const o = store.get(key);
          if (!o) continue;
          const before = o.versions.length;
          while (o.versions[0]?.deleteMarker) o.versions.shift();
          if (o.versions.length !== before) n++;
        }
        return { succeeded: n, skipped: [], failed: [] };
      }

      // 転送
      case 'transfer_subscribe':
        this.transferChannel = a.onEvent;
        return [...this.jobs.values()].map((j) => j.job);
      case 'mock_register_selection': {
        const id = crypto.randomUUID();
        const items = a.items as MockSelection['items'];
        this.selections.set(id, { items });
        return { selectionId: id, items } satisfies Selection;
      }
      case 'pick_upload_files':
      case 'pick_download_dir':
        // ブラウザではネイティブのダイアログを開けないため、呼び出し側が input 要素を使う
        return null;
      case 'upload_prepare': {
        const sel = this.selections.get(a.selectionId);
        if (!sel) fail('NOT_FOUND', '選択が無効になりました。もう一度選択してください');
        const store = this.store(a.connectionId);
        const files = sel.items
          .filter((i) => !i.isDir && i.name !== '.DS_Store')
          .map((i) => ({ key: `${a.prefix}${i.name}`, size: i.size }));
        const conflicts = files.filter((f) => {
          const o = store.get(f.key);
          return o && this.isLive(o);
        });
        const planId = crypto.randomUUID();
        this.plans.set(planId, {
          connectionId: a.connectionId,
          prefix: a.prefix,
          files,
          conflicts: new Set(conflicts.map((c) => c.key)),
        });
        return {
          planId,
          fileCount: files.length,
          totalBytes: files.reduce((s, f) => s + f.size, 0),
          conflicts: conflicts.map((c) => {
            const o = store.get(c.key);
            return {
              key: c.key,
              localSize: c.size,
              remoteSize: (o && this.current(o)?.size) ?? 0,
              remoteModified: iso((o && this.current(o)?.date) ?? NOW),
            };
          }),
          excluded: sel.items
            .filter((i) => i.name === '.DS_Store')
            .map((i) => ({ name: i.name, reason: 'ignored' })),
          versioningEnabled: this.bucket(a.connectionId).versioning !== 'disabled',
        } satisfies UploadPlan;
      }
      case 'upload_start': {
        const plan = this.plans.get(a.planId);
        if (!plan) fail('NOT_FOUND', 'アップロードの準備が無効になりました。もう一度お試しください');
        this.plans.delete(a.planId);
        const decisions = (a.decisions ?? {}) as Decisions;
        const store = this.store(plan.connectionId);
        const files: { key: string; size: number }[] = [];
        for (const f of plan.files) {
          const d = plan.conflicts.has(f.key)
            ? 'all' in decisions
              ? decisions.all
              : ((decisions as Record<string, string>)[f.key] ?? 'replace')
            : 'replace';
          if (d === 'skip') continue;
          if (d === 'keepBoth') {
            const dot = f.key.lastIndexOf('.');
            let i = 1;
            let key = f.key;
            while (store.has(key)) {
              key =
                dot > f.key.lastIndexOf('/')
                  ? `${f.key.slice(0, dot)} (${i})${f.key.slice(dot)}`
                  : `${f.key} (${i})`;
              i++;
            }
            files.push({ key, size: f.size });
          } else {
            files.push(f);
          }
        }
        const b = this.bucket(plan.connectionId);
        const job: TransferJob = {
          jobId: crypto.randomUUID(),
          kind: 'upload',
          connectionId: plan.connectionId,
          title: `${files.length} 件をアップロード中`,
          status: 'queued',
          totalFiles: files.length,
          doneFiles: 0,
          failedFiles: 0,
          totalBytes: Math.max(
            1,
            files.reduce((s, f) => s + f.size, 0),
          ),
          doneBytes: 0,
          currentName: files[0] ? baseName(files[0].key) : null,
          bytesPerSec: 0,
          etaSec: null,
          destination: `${b.name}/${plan.prefix}`,
        };
        return this.runTransfer(job, () => {
          for (const f of files) {
            const existing = store.get(f.key);
            const version = {
              id: versionId(),
              date: new Date(),
              size: f.size,
              deleteMarker: false,
              storageClass: 'STANDARD' as const,
            };
            if (existing && b.versioning !== 'disabled') existing.versions.unshift(version);
            else {
              store.set(f.key, {
                key: f.key,
                contentType: contentTypeOf(f.key),
                etag: versionId().toLowerCase().padEnd(32, '0').slice(0, 32),
                metadata: { 's3drive-mtime': iso(new Date()) },
                versions: [version],
              });
            }
          }
        });
      }
      case 'download_start': {
        const store = this.store(a.connectionId);
        const targets = a.targets as Target[];
        const items = this.expand(a.connectionId, targets).filter((o) => !o.key.endsWith('/'));
        const archived = targets.find((t) => {
          const o = store.get(t.key);
          return !t.isFolder && o && ['archived', 'inProgress'].includes(this.restoreState(o).state);
        });
        if (archived) fail('INVALID_OBJECT_STATE', '取り出しが必要です');
        const first = targets[0];
        const job: TransferJob = {
          jobId: crypto.randomUUID(),
          kind: 'download',
          connectionId: a.connectionId,
          title:
            targets.length === 1 && first
              ? `「${baseName(first.key)}」をダウンロード中`
              : `${items.length} 件をダウンロード中`,
          status: 'queued',
          totalFiles: items.length,
          doneFiles: 0,
          failedFiles: 0,
          totalBytes: Math.max(
            1,
            items.reduce((s, o) => s + (this.current(o)?.size ?? 0), 0),
          ),
          doneBytes: 0,
          currentName: items[0] ? baseName(items[0].key) : null,
          bytesPerSec: 0,
          etaSec: null,
          destination: '~/Downloads',
        };
        return this.runTransfer(job, () => {});
      }
      case 'download_reveal':
        return null;
      case 'job_cancel': {
        const j = this.jobs.get(a.jobId);
        if (j) {
          clearInterval(j.timer);
          j.job.status = 'canceled';
          this.emitJob(j.job);
        }
        return null;
      }
      case 'transfer_retry':
        return a.jobId;
      case 'transfer_clear_finished':
        for (const [id, j] of this.jobs)
          if (['succeeded', 'failed', 'canceled'].includes(j.job.status)) this.jobs.delete(id);
        return null;

      // 検索
      case 'search_query': {
        const q = a.query as SearchQuery;
        const store = this.store(a.connectionId);
        const text = q.text.trim().normalize('NFKC').toLowerCase();
        const hasFilters = Boolean(q.kind || q.ext || q.size || q.date || q.storageClass);
        const out: SearchEntry[] = [];
        const folders = new Set<string>();
        for (const o of store.values()) {
          if (!this.isLive(o)) continue;
          const name = baseName(o.key);
          if (o.key.endsWith('/')) {
            if (text && !hasFilters && name.normalize('NFKC').toLowerCase().includes(text))
              folders.add(o.key);
            continue;
          }
          if (text && !name.normalize('NFKC').toLowerCase().includes(text)) continue;
          const v = this.current(o);
          const size = v?.size ?? 0;
          if (q.kind && fileKind(name) !== q.kind) continue;
          if (q.ext && extensionOf(name) !== q.ext.replace(/^\./, '').toLowerCase()) continue;
          if (q.size === 'lt1' && size >= 1e6) continue;
          if (q.size === '1to100' && (size < 1e6 || size >= 1e8)) continue;
          if (q.size === 'gt100' && size < 1e8) continue;
          const days = (NOW.getTime() - (v?.date ?? NOW).getTime()) / 864e5;
          if (q.date === '7d' && days > 7) continue;
          if (q.date === '30d' && days > 30) continue;
          if (q.date === 'year' && (v?.date ?? NOW).getFullYear() !== NOW.getFullYear()) continue;
          if (q.storageClass && v?.storageClass !== q.storageClass) continue;
          out.push({ entry: this.fileEntry(o), parent: parentOf(o.key) });
        }
        if (!text && !hasFilters) {
          return { entries: [], total: 0, index: this.indexStatus(store.size) };
        }
        // Rust 側と同じく、フォルダは最初のページの先頭に最大 200 件を置き、offset・limit はファイルだけに
        // 適用する。並び順は一覧と同じ規則（04 §10.2）
        const folderEntries: SearchEntry[] = sortEntries(
          [...folders].map((k) => ({
            type: 'folder' as const,
            key: k,
            name: baseName(k),
            lastModified: null,
            deleted: false,
          })),
          q.sort,
        )
          .slice(0, 200)
          .map((entry) => ({ entry, parent: parentOf(entry.key) }));
        const parents = new Map(out.map((r) => [r.entry.key, r.parent]));
        const files = sortEntries(
          out.map((r) => r.entry),
          q.sort,
        )
          .slice(q.offset, q.offset + q.limit)
          .map((entry) => ({ entry, parent: parents.get(entry.key) ?? '' }));
        const entries = [...(q.offset === 0 ? folderEntries : []), ...files];
        return {
          entries,
          total: out.length + folderEntries.length,
          index: this.indexStatus(store.size),
        };
      }
      case 'search_index_status':
        return this.indexStatus(this.store(a.connectionId).size);
      case 'search_index_rebuild': {
        const jobId = crypto.randomUUID();
        const ch = a.onEvent as Channel<unknown>;
        this.store(a.connectionId);
        setTimeout(() => {
          this.indexed.add(a.connectionId);
          ch.onmessage({
            event: 'finished',
            data: {
              jobId,
              status: {
                state: 'ready',
                objectCount: this.store(a.connectionId).size,
                lastScanAt: iso(new Date()),
                sizeBytes: 4_200_000,
                progress: null,
              },
            },
          });
        }, this.tick * 3);
        return jobId;
      }
      case 'search_index_delete':
        return null;

      // メトリクス・コスト
      case 'metrics_storage': {
        const b = this.bucket(a.connectionId);
        const byClass: Record<string, number> = {};
        let total = 0;
        for (const [k, gb] of Object.entries(b.usageGb)) {
          const bytes = Math.round((gb ?? 0) * 1e9);
          byClass[k] = bytes;
          total += bytes;
        }
        if (total > 0) {
          return {
            source: 'cloudwatch',
            asOf: iso(new Date(NOW.getTime() - 864e5)),
            totalBytes: total,
            objectCount: b.objectCount,
            byClass,
          };
        }
        if (!this.indexed.has(a.connectionId)) {
          return { source: 'none', asOf: null, totalBytes: 0, objectCount: null, byClass: {} };
        }
        // CloudWatch のメトリクスがなければインデックス（現行バージョン）から集計する
        let count = 0;
        for (const o of this.store(a.connectionId).values()) {
          const v = this.current(o);
          if (!v || v.deleteMarker || o.key.endsWith('/')) continue;
          const size = v.size ?? 0;
          byClass[v.storageClass] = (byClass[v.storageClass] ?? 0) + size;
          total += size;
          count += 1;
        }
        return { source: 'index', asOf: null, totalBytes: total, objectCount: count, byClass };
      }
      case 'cost_summary':
        this.bucket(a.connectionId);
        return this.costs.get(a.connectionId) ?? null;
      case 'cost_refresh': {
        if (!this.settings.cost.useCostExplorer)
          fail('COST_UNAVAILABLE', 'Cost Explorer は設定で無効になっています');
        const b = this.bucket(a.connectionId);
        const storage = Object.entries(b.usageGb).reduce(
          (s, [k, gb]) => s + (gb ?? 0) * (PRICE[k as StorageClass] ?? 0),
          0,
        );
        const mtd = storage + b.cost.requests + b.cost.transfer + b.cost.retrieval;
        const day = NOW.getDate();
        const daily = Array.from({ length: 30 }, (_, i) =>
          i < day ? Number((b.cost.dailyBase * (0.8 + 0.4 * Math.abs(Math.sin(i * 1.7)))).toFixed(3)) : null,
        );
        const summary: CostSummary = {
          scope: { kind: 'accountRegion', region: b.region },
          month: `${NOW.getFullYear()}-${String(NOW.getMonth() + 1).padStart(2, '0')}`,
          monthToDate: mtd,
          prevMonthSamePeriod: b.cost.prevMonth,
          breakdown: {
            storage,
            requests: b.cost.requests,
            transfer: b.cost.transfer,
            retrieval: b.cost.retrieval,
            other: 0,
          },
          daily,
          forecastMonthEnd: (mtd / day) * 30,
          currency: 'USD',
          fetchedAt: iso(new Date()),
        };
        this.costs.set(a.connectionId, summary);
        return summary;
      }
      case 'pricing_get':
        return { region: a.region, perGbMonth: PRICE, source: 'api', asOf: iso(NOW) };

      // 設定・アプリ
      case 'settings_get':
        return this.settings;
      case 'settings_update': {
        const patch = a.patch as Record<string, Record<string, unknown>>;
        const next = structuredClone(this.settings) as unknown as Record<string, Record<string, unknown>>;
        for (const [section, values] of Object.entries(patch)) {
          // Rust 側と同じく、ダウンロード先のパスは受け取らない（05 §3.9）
          const { downloadDir: _ignored, ...rest } = values;
          next[section] = { ...next[section], ...(section === 'general' ? rest : values) };
        }
        this.settings = next as unknown as Settings;
        void emit('settings://changed', this.settings);
        return this.settings;
      }
      case 'app_set_theme':
        this.settings.general.appearance = a.theme as never;
        void emit('settings://changed', this.settings);
        return null;
      case 'app_choose_download_dir':
        return this.settings;
      case 'app_startup_info':
        return { session: this.session, dbRecreated: false, version: '0.1.0' };
      case 'app_check_update':
        return null;
      case 'menu_update_state':
      case 'app_open_settings':
      case 'app_open_logs':
      case 'app_clear_cache':
      case 'app_install_update':
      case 'app_notify':
        return null;
      default:
        if (cmd.startsWith('plugin:')) return null;
        fail('INTERNAL', `モックに未実装のコマンドです: ${cmd}`);
    }
  }
}

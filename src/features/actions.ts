// 操作（ダウンロード・アップロード・削除・移動など）の入口。ツールバー・メニュー・ショートカット・
// インスペクタ・ダイアログはここを呼ぶ（01 §6、03 §5〜§10）。

import { ArchiveRestore, Copy, FolderInput, Layers, Pencil, RotateCcw, Trash2 } from 'lucide-react';
import { queryClient } from '@/app/query-client';
import { qk } from '@/app/query-keys';
import { toast } from '@/components/ui/toaster';
import { formatDate } from '@/lib/format';
import { ja } from '@/lib/i18n/ja';
import type {
  BatchResult,
  BucketInfo,
  Decisions,
  DownloadDestination,
  Entry,
  RemoteConflict,
  RestoreTier,
  Selection,
  StorageClass,
  Target,
  UploadConflict,
} from '@/lib/ipc';
import * as ipc from '@/lib/ipc';
import { requireMock } from '@/lib/ipc/mock-loader';
import { isNative } from '@/lib/platform';
import { classLabel } from '@/lib/storage-class';
import { parentPrefix, useNavStore } from '@/stores/nav';
import { filterCount, useUiStore } from '@/stores/ui';
import { runBatch } from './batch';
import { isArchived, selectedEntries } from './context';
import { showError } from './errors';

export function toTarget(e: Entry): Target {
  return { key: e.key, isFolder: e.type === 'folder' };
}

function conn(): string | null {
  return useNavStore.getState().connectionId;
}

function bucketInfo(connId: string): BucketInfo | undefined {
  return queryClient.getQueryData<BucketInfo>(qk.bucket(connId));
}

export function isVersioned(connId: string | null): boolean {
  if (!connId) return false;
  const v = bucketInfo(connId)?.versioning;
  return v === 'enabled' || v === 'suspended';
}

/** 変更操作のあとに無効化するクエリ（01 §5.3）。 */
export function changedKeys(connId: string) {
  return [
    ['objects', connId],
    ['object', connId],
    ['versions', connId],
    ['folderSummary', connId],
    ['folderChildren', connId],
    ['search', connId],
    ['indexStatus', connId],
  ];
}

function invalidateAll(connId: string) {
  for (const key of changedKeys(connId)) void queryClient.invalidateQueries({ queryKey: key });
}

/** 一覧から楽観的に取り除く（削除・移動。失敗時は無効化で元に戻る。01 §5.3）。 */
function removeFromListing(connId: string, keys: string[]) {
  const set = new Set(keys);
  queryClient.setQueriesData<{ pages: ipc.ListPage[]; pageParams: unknown[] }>(
    { queryKey: ['objects', connId] },
    (data) =>
      data
        ? {
            ...data,
            pages: data.pages.map((p) => ({ ...p, entries: p.entries.filter((e) => !set.has(e.key)) })),
          }
        : data,
  );
}

function baseName(key: string): string {
  const trimmed = key.replace(/\/$/, '');
  return trimmed.slice(trimmed.lastIndexOf('/') + 1);
}

// ---- 開く・ナビゲーション ---------------------------------------------------------------

/** 項目を開く（フォルダは移動、ファイルはダウンロード。03 §5.4）。 */
export function openEntry(e: Entry) {
  if (e.deleted) return;
  if (e.type === 'folder') {
    const ui = useUiStore.getState();
    if (ui.query || filterCount(ui.filters) > 0) ui.clearSearch();
    useNavStore.getState().navigate(e.key);
    ui.setSelection([]);
    return;
  }
  void download([e]);
}

export function openSelection() {
  const [first, ...rest] = selectedEntries();
  if (!first) return;
  if (rest.length === 0) openEntry(first);
  else void download([first, ...rest]);
}

export function reload() {
  const connId = conn();
  if (!connId) return;
  invalidateAll(connId);
  void queryClient.invalidateQueries({ queryKey: qk.bucket(connId) });
}

// ---- ダウンロード -----------------------------------------------------------------------

/** 取り出していないアーカイブを含むときは DLG-07 を開いて中止する（04 §5.2）。 */
function guardArchived(items: Entry[]): boolean {
  const archived = items.filter(isArchived);
  if (archived.length === 0) return true;
  useUiStore.getState().openDialog({ type: 'restore', items: archived });
  return false;
}

export async function download(items: Entry[], pickLocation = false) {
  const connId = conn();
  const live = items.filter((e) => !e.deleted);
  if (!connId || live.length === 0 || !guardArchived(live)) return;
  let destination: DownloadDestination = 'default';
  if (pickLocation && isNative()) {
    try {
      const picked = await ipc.transfers.pickDownloadDir();
      if (!picked) return;
      destination = { selectionId: picked.selectionId };
    } catch (e) {
      showError(e, ja.verbs.download);
      return;
    }
  }
  try {
    await ipc.transfers.startDownload(connId, live.map(toTarget), destination);
  } catch (e) {
    if (ipc.errorCode(e) === 'INVALID_OBJECT_STATE') {
      useUiStore.getState().openDialog({ type: 'restore', items: live.filter((x) => x.type === 'file') });
      return;
    }
    showError(e, ja.verbs.download, () => void download(items, pickLocation));
  }
}

export async function downloadVersion(key: string, versionId: string) {
  const connId = conn();
  if (!connId) return;
  try {
    await ipc.transfers.startDownload(connId, [{ key, isFolder: false, versionId }], 'default');
  } catch (e) {
    showError(e, ja.verbs.download);
  }
}

// ---- アップロード -----------------------------------------------------------------------

/** ブラウザ（モック）でのファイル選択。 */
function pickBrowserFiles(directories: boolean): Promise<File[]> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.multiple = true;
    if (directories) input.webkitdirectory = true;
    input.addEventListener('change', () => resolve(Array.from(input.files ?? [])));
    input.addEventListener('cancel', () => resolve([]));
    input.click();
  });
}

async function pickSelection(directories: boolean): Promise<Selection | null> {
  if (isNative()) return ipc.transfers.pickUploadFiles(directories);
  const files = await pickBrowserFiles(directories);
  if (files.length === 0) return null;
  const { registerBrowserFiles } = await requireMock();
  return registerBrowserFiles(files);
}

/** アップロード（⌘U／⌥⌘U、MNU-02。03 §5.5）。 */
export async function upload(directories = false) {
  const connId = conn();
  if (!connId) return;
  try {
    const selection = await pickSelection(directories);
    if (!selection) return;
    await uploadSelection(selection.selectionId, useNavStore.getState().prefix);
  } catch (e) {
    showError(e, ja.verbs.upload);
  }
}

/** 選択（ファイル選択・ドロップ）をプレフィックスへアップロードする（04 §4.1）。 */
export async function uploadSelection(selectionId: string, prefix: string) {
  const connId = conn();
  if (!connId) return;
  try {
    const plan = await ipc.transfers.prepareUpload(connId, prefix, selectionId);
    if (plan.excluded.length > 0) {
      toast.show({
        tone: 'warning',
        title: ja.toast.excluded(plan.excluded.length),
        actions: [
          {
            label: ja.common.details,
            onClick: () =>
              useUiStore.getState().openDialog({
                type: 'details',
                title: ja.toast.excluded(plan.excluded.length),
                lines: plan.excluded.map((x) => `${x.name}: ${EXCLUDE_REASON[x.reason]}`),
              }),
          },
        ],
      });
    }
    if (plan.fileCount === 0) return;
    let decisions: Decisions = {};
    if (plan.conflicts.length > 0) {
      const resolved = await resolveConflicts(plan.conflicts, plan.versioningEnabled);
      if (!resolved) return;
      decisions = resolved;
    }
    await ipc.transfers.startUpload(plan.planId, decisions);
  } catch (e) {
    showError(e, ja.verbs.upload);
  }
}

const EXCLUDE_REASON: Record<ipc.ExcludeReason, string> = {
  symlink: 'シンボリックリンク',
  ignored: '除外の設定に一致',
  keyTooLong: 'キーが 1,024 バイトを超える',
  invalidChar: '使用できない文字を含む',
  unreadable: '読み取れない',
};

/** DLG-08 で同名の項目の扱いを決める。中止なら null。 */
export function resolveConflicts(
  conflicts: RemoteConflict[] | UploadConflict[],
  versioned: boolean,
): Promise<Decisions | null> {
  return new Promise((resolve) => {
    useUiStore.getState().openDialog({
      type: 'conflict',
      conflicts,
      versioned,
      resolve: (result) => {
        useUiStore.getState().closeDialog();
        resolve(result);
      },
    });
  });
}

// ---- 新規フォルダ・名前の変更 -------------------------------------------------------------

export async function createFolder(name: string): Promise<boolean> {
  const connId = conn();
  if (!connId) return false;
  const prefix = useNavStore.getState().prefix;
  try {
    const entry = await ipc.objects.createFolder(connId, prefix, name);
    await queryClient.invalidateQueries({ queryKey: qk.objects(connId, prefix) });
    void queryClient.invalidateQueries({ queryKey: ['folderChildren', connId] });
    useUiStore.getState().setSelection([entry.key]);
    toast.show({ tone: 'success', title: ja.dialog.newFolder.created(name) });
    return true;
  } catch (e) {
    showError(e, ja.verbs.createFolder);
    return false;
  }
}

export async function rename(item: Entry, newName: string) {
  const connId = conn();
  if (!connId) return;
  await runBatch({
    verb: ja.verbs.rename,
    icon: Pencil,
    runningTitle: ja.dialog.rename.title,
    start: (onEvent) => ipc.objects.rename(connId, toTarget(item), newName, onEvent),
    success: () => ({ title: ja.toast.renamed }),
    invalidate: changedKeys(connId),
  });
}

// ---- 削除・移動・クラス変更・取り出し ------------------------------------------------------

/**
 * 完了のトーストの件数（DLG-02・03 の「{n} 項目」）。すべて成功した場合は、確認ダイアログと同じく選択した
 * 項目の数にする（バックエンドの件数はフォルダの配下のオブジェクトを数えるため）。スキップがあれば処理した件数。
 */
export function selectedCount(items: Entry[], result: BatchResult): number {
  return result.skipped.length === 0 && result.failed.length === 0 ? items.length : result.succeeded;
}

export async function deleteEntries(items: Entry[], allVersions: boolean): Promise<BatchResult | null> {
  const connId = conn();
  if (!connId || items.length === 0) return null;
  const versioned = isVersioned(connId);
  // 削除済みの項目を表示中で削除マーカーが作られる場合は、行が「削除マーカー」として残る
  if (!(versioned && !allVersions && useUiStore.getState().showDeleted)) {
    removeFromListing(
      connId,
      items.map((e) => e.key),
    );
  }
  useUiStore.getState().setSelection([]);
  return runBatch({
    verb: ja.verbs.delete,
    icon: Trash2,
    runningTitle: ja.dialog.delete.titleMany(items.length),
    start: (onEvent) => ipc.objects.delete(connId, items.map(toTarget), allVersions, onEvent),
    success: (r) => ({
      title: ja.dialog.delete.done(selectedCount(items, r)),
      description: versioned && !allVersions ? ja.dialog.delete.doneMarker : ja.dialog.delete.doneAll,
    }),
    invalidate: changedKeys(connId),
  });
}

export async function move(items: Entry[], destPrefix: string) {
  const connId = conn();
  if (!connId || items.length === 0) return;
  const targets = items.map(toTarget);
  let decisions: Decisions = {};
  try {
    const conflicts = await ipc.objects.findConflicts(connId, targets, destPrefix);
    if (conflicts.length > 0) {
      const resolved = await resolveConflicts(conflicts, isVersioned(connId));
      if (!resolved) return;
      decisions = resolved;
    }
  } catch (e) {
    showError(e, ja.verbs.move);
    return;
  }
  removeFromListing(
    connId,
    items.map((e) => e.key),
  );
  useUiStore.getState().setSelection([]);
  await runBatch({
    verb: ja.verbs.move,
    icon: FolderInput,
    runningTitle: ja.dialog.move.titleMany(items.length),
    start: (onEvent) => ipc.objects.move(connId, targets, destPrefix, decisions, onEvent),
    success: (r) => ({
      title: ja.dialog.move.done(selectedCount(items, r)),
      description: ja.dialog.move.doneTo(destPrefix),
    }),
    invalidate: changedKeys(connId),
  });
}

export async function changeStorageClass(items: Entry[], storageClass: StorageClass) {
  const connId = conn();
  if (!connId || items.length === 0) return;
  await runBatch({
    verb: ja.verbs.storageClass,
    icon: Layers,
    runningTitle: ja.dialog.storageClass.title,
    start: (onEvent) => ipc.objects.changeStorageClass(connId, items.map(toTarget), storageClass, onEvent),
    success: () => ({ title: ja.dialog.storageClass.done, description: classLabel(storageClass) }),
    invalidate: changedKeys(connId),
  });
}

export async function requestRestore(items: Entry[], tier: RestoreTier, days?: number) {
  const connId = conn();
  if (!connId || items.length === 0) return;
  try {
    const result = await ipc.objects.requestRestore(connId, items.map(toTarget), tier, days);
    invalidateAll(connId);
    if (result.failed.length > 0 && result.succeeded === 0 && result.failed[0]) {
      showError(result.failed[0].error, ja.verbs.restore);
      return;
    }
    toast.show({
      icon: ArchiveRestore,
      tone: result.failed.length ? 'warning' : 'success',
      title: result.failed.length
        ? ja.toast.partialFailure(items.length, result.failed.length, ja.verbs.restore)
        : ja.dialog.restore.requested,
      description: ja.dialog.restore.requestedDesc,
    });
  } catch (e) {
    showError(e, ja.verbs.restore);
  }
}

// ---- バージョン・削除済みの項目 ----------------------------------------------------------

export async function restoreVersion(key: string, versionId: string, date: string) {
  const connId = conn();
  if (!connId) return;
  try {
    await ipc.versions.restore(connId, key, versionId);
    invalidateAll(connId);
    toast.show({
      icon: RotateCcw,
      tone: 'success',
      title: ja.toast.versionRestored,
      description: ja.toast.versionRestoredDesc(formatDate(date)),
    });
  } catch (e) {
    showError(e, ja.verbs.versionRestore);
  }
}

export async function deleteVersion(key: string, versionId: string) {
  const connId = conn();
  if (!connId) return;
  try {
    await ipc.versions.remove(connId, key, versionId);
    invalidateAll(connId);
    toast.show({ tone: 'success', title: ja.dialog.deleteVersion.done });
  } catch (e) {
    showError(e, ja.verbs.versionDelete);
  }
}

/** 削除済みの項目を復元する（最新の削除マーカーを削除）。 */
export async function undelete(items: Entry[]) {
  const connId = conn();
  if (!connId || items.length === 0) return;
  try {
    const result = await ipc.versions.undelete(
      connId,
      items.map((e) => e.key),
    );
    invalidateAll(connId);
    if (result.failed.length > 0) {
      toast.show({
        tone: 'warning',
        title: ja.toast.partialFailure(items.length, result.failed.length, '復元'),
        persistent: true,
      });
    } else {
      toast.show({ icon: RotateCcw, tone: 'success', title: ja.toast.undeleted(result.succeeded) });
    }
  } catch (e) {
    showError(e, '復元');
  }
}

// ---- キーのコピー ------------------------------------------------------------------------

export async function copyKeys(items: Entry[]) {
  if (items.length === 0) return;
  const text = items.map((e) => e.key).join('\n');
  try {
    if (isNative()) {
      const { writeText } = await import('@tauri-apps/plugin-clipboard-manager');
      await writeText(text);
    } else {
      await navigator.clipboard.writeText(text);
    }
    toast.show({ icon: Copy, tone: 'success', title: ja.toast.copied });
  } catch (e) {
    showError(e, 'コピー');
  }
}

// ---- 選択に対する操作（メニュー・ショートカットの入口） --------------------------------------

export type SelectionAction =
  | 'open'
  | 'download'
  | 'downloadTo'
  | 'move'
  | 'rename'
  | 'storageClass'
  | 'restore'
  | 'versions'
  | 'copyKey'
  | 'delete'
  | 'undelete'
  | 'purge';

export function runSelectionAction(action: SelectionAction, items: Entry[] = selectedEntries()) {
  const ui = useUiStore.getState();
  const [first] = items;
  if (!first) return;
  switch (action) {
    case 'open':
      openEntry(first);
      return;
    case 'download':
      void download(items);
      return;
    case 'downloadTo':
      void download(items, true);
      return;
    case 'move':
      ui.openDialog({ type: 'move', items });
      return;
    case 'rename':
      if (items.length === 1) ui.openDialog({ type: 'rename', item: first });
      return;
    case 'storageClass':
      ui.openDialog({ type: 'storageClass', items });
      return;
    case 'restore': {
      const archived = items.filter(isArchived);
      if (archived.length) ui.openDialog({ type: 'restore', items: archived });
      return;
    }
    case 'versions':
      ui.setSelection([first.key]);
      ui.setInspectorVisible(true);
      ui.setInspectorTab('versions');
      return;
    case 'copyKey':
      void copyKeys(items);
      return;
    case 'delete':
      ui.openDialog({ type: 'delete', items });
      return;
    case 'undelete':
      void undelete(items);
      return;
    case 'purge':
      ui.openDialog({ type: 'delete', items });
      return;
  }
}

// ---- 表示の切り替え ------------------------------------------------------------------------

/** 表示の設定（表示モード・並べ替え・インスペクタ・サイドバーの幅）を保存する（01 §5.4）。 */
export function persistView() {
  const { viewMode, sort, inspectorVisible, sidebarWidth } = useUiStore.getState();
  void ipc.app
    .updateSettings({ view: { mode: viewMode, sort, inspector: inspectorVisible, sidebarWidth } })
    .catch(() => {});
}

export async function toggleShowHidden() {
  const settings = queryClient.getQueryData<ipc.Settings>(qk.settings);
  const next = !(settings?.general.showHidden ?? false);
  try {
    const updated = await ipc.app.updateSettings({ general: { showHidden: next } });
    queryClient.setQueryData(qk.settings, updated);
  } catch (e) {
    showError(e, '設定を変更');
  }
}

export function goUp() {
  const nav = useNavStore.getState();
  if (!nav.prefix) return;
  const from = nav.prefix;
  nav.navigate(parentPrefix(from));
  useUiStore.getState().setSelection([from]);
}

export { baseName };

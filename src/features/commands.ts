// メニューバーの項目・キーボードショートカットの処理（03 §9.4、§10）。
// ネイティブではメニューのアクセラレータが OS で処理されて `menu://action` が届くため、JS ではメニューにない
// キー（矢印・Return・Esc・⌘A）だけを扱う。モック（ブラウザ）ではすべてのショートカットを JS で扱う。

import { Info } from 'lucide-react';
import { useEffect } from 'react';
import { queryClient } from '@/app/query-client';
import { qk } from '@/app/query-keys';
import { toast } from '@/components/ui/toaster';
import { focusSearchInput } from '@/features/browser/toolbar';
import { ja } from '@/lib/i18n/ja';
import type { MenuState, Settings } from '@/lib/ipc';
import * as ipc from '@/lib/ipc';
import { isNative } from '@/lib/platform';
import { useNavStore } from '@/stores/nav';
import { filterCount, useUiStore } from '@/stores/ui';
import {
  goUp,
  openSelection,
  persistView,
  reload,
  runSelectionAction,
  toggleShowHidden,
  upload,
} from './actions';
import { isArchived, selectedEntries, visibleOrder } from './context';
import { showError } from './errors';

export async function checkForUpdate(silent = false) {
  try {
    const info = await ipc.app.checkUpdate();
    if (info) useUiStore.getState().openDialog({ type: 'update', version: info.version, notes: info.notes });
    else if (!silent) toast.show({ icon: Info, title: ja.dialog.update.none });
  } catch (e) {
    if (!silent) showError(e, 'アップデートを確認');
  }
}

/** メニューの項目 ID ごとの処理。 */
export function handleCommand(id: string) {
  const ui = useUiStore.getState();
  const nav = useNavStore.getState();
  const files = ui.view === 'files' && nav.connectionId !== null;
  switch (id) {
    case 'file.new_folder':
      if (files) ui.openDialog({ type: 'newFolder' });
      return;
    case 'tray.upload':
      if (nav.connectionId) {
        ui.setView('files');
        void upload();
      }
      return;
    case 'file.upload':
      if (files) void upload();
      return;
    case 'file.upload_folder':
      if (files) void upload(true);
      return;
    case 'file.download':
      if (files) runSelectionAction('download');
      return;
    case 'file.download_to':
      if (files) runSelectionAction('downloadTo');
      return;
    case 'file.rename':
      if (files) runSelectionAction('rename');
      return;
    case 'file.move':
      if (files) runSelectionAction('move');
      return;
    case 'file.storage_class':
      if (files) runSelectionAction('storageClass');
      return;
    case 'file.restore':
      if (files) runSelectionAction('restore');
      return;
    case 'file.delete':
      if (files) runSelectionAction('delete');
      return;
    case 'edit.copy_key':
      if (files) runSelectionAction('copyKey');
      return;
    case 'edit.find':
      if (files) focusSearchInput();
      return;
    case 'view.grid':
    case 'view.list':
      ui.setViewMode(id === 'view.grid' ? 'grid' : 'list');
      persistView();
      return;
    case 'view.inspector':
      ui.setInspectorVisible(!ui.inspectorVisible);
      persistView();
      return;
    case 'view.filters':
      ui.setFiltersOpen(!ui.filtersOpen);
      return;
    case 'view.hidden':
      void toggleShowHidden();
      return;
    case 'view.deleted':
      ui.setShowDeleted(!ui.showDeleted);
      return;
    case 'view.reload':
      if (ui.view === 'dashboard' && nav.connectionId) {
        // Cost Explorer には問い合わせない（04 §13.4）
        void queryClient.invalidateQueries({ queryKey: qk.storageMetrics(nav.connectionId) });
        void queryClient.invalidateQueries({ queryKey: qk.bucket(nav.connectionId) });
      } else {
        reload();
      }
      return;
    case 'go.back':
      nav.goBack();
      ui.setSelection([]);
      return;
    case 'go.forward':
      nav.goForward();
      ui.setSelection([]);
      return;
    case 'go.up':
      if (files) goUp();
      return;
    case 'go.open':
      if (files) openSelection();
      return;
    case 'go.dashboard':
      ui.setView('dashboard');
      return;
    case 'app.check_update':
      void checkForUpdate();
      return;
    case 'app.settings':
      void ipc.app.openSettings();
      if (!isNative()) window.open('/settings.html', 'settings', 'width=560,height=480');
      return;
  }
}

const isMac =
  typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

/** メニューのアクセラレータに相当するキー（モックのみ）。 */
function menuShortcut(e: KeyboardEvent): string | null {
  const mod = isMac ? e.metaKey : e.ctrlKey;
  if (!mod) return null;
  const { shiftKey: shift, altKey: alt } = e;
  switch (e.code) {
    case 'KeyU':
      return alt ? 'file.upload_folder' : 'file.upload';
    case 'KeyN':
      return shift ? 'file.new_folder' : null;
    case 'KeyD':
      return shift ? 'file.download_to' : 'file.download';
    case 'Backspace':
    case 'Delete':
      return 'file.delete';
    case 'KeyC':
      return alt ? 'edit.copy_key' : null;
    case 'KeyF':
      return alt ? 'view.filters' : 'edit.find';
    case 'Digit1':
      return 'view.grid';
    case 'Digit2':
      return 'view.list';
    case 'KeyI':
      return alt ? 'view.inspector' : null;
    case 'Period':
      return shift ? 'view.hidden' : null;
    case 'KeyR':
      return 'view.reload';
    case 'BracketLeft':
      return 'go.back';
    case 'BracketRight':
      return 'go.forward';
    case 'ArrowUp':
      return 'go.up';
    case 'ArrowDown':
      return 'go.open';
    case 'Comma':
      return 'app.settings';
    default:
      return null;
  }
}

function isTextInput(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName);
}

/** 入力欄で標準動作に任せるキー（03 §10）。 */
const TEXT_KEYS = new Set([
  'KeyA',
  'Backspace',
  'Delete',
  'ArrowUp',
  'ArrowDown',
  'ArrowLeft',
  'ArrowRight',
  'Enter',
]);

export function onKeyDown(e: KeyboardEvent) {
  const ui = useUiStore.getState();
  if (ui.dialog || e.defaultPrevented || e.isComposing) return;
  const inInput = isTextInput(e.target);
  const mod = isMac ? e.metaKey : e.ctrlKey;
  if (inInput && TEXT_KEYS.has(e.code)) return;
  // メニューにある項目（モックのみ JS で処理する）
  if (!isNative()) {
    const id = menuShortcut(e);
    if (id) {
      e.preventDefault();
      handleCommand(id);
      return;
    }
  }
  if (inInput || ui.view !== 'files') return;
  const order = visibleOrder();
  if (mod && e.code === 'KeyA') {
    e.preventDefault();
    ui.selectAll(order);
    return;
  }
  if (e.metaKey || e.ctrlKey || e.altKey) return;
  switch (e.key) {
    case 'ArrowDown':
    case 'ArrowUp':
      e.preventDefault();
      ui.moveSelection(e.key === 'ArrowDown' ? 1 : -1, e.shiftKey, order);
      return;
    case 'ArrowRight':
    case 'ArrowLeft':
      if (ui.viewMode === 'grid') {
        e.preventDefault();
        ui.moveSelection(e.key === 'ArrowRight' ? 1 : -1, e.shiftKey, order);
      }
      return;
    case 'Enter':
      if (ui.selection.keys.length === 1) {
        e.preventDefault();
        runSelectionAction('rename');
      }
      return;
    case 'Escape':
      if (ui.query || filterCount(ui.filters)) ui.clearSearch();
      else ui.setSelection([]);
      return;
  }
}

/** メニューの有効・無効に使う状態。 */
export function menuState(settings: Settings | undefined): MenuState {
  const ui = useUiStore.getState();
  const nav = useNavStore.getState();
  const selected = selectedEntries();
  return {
    view: ui.view,
    selectionCount: selected.length,
    hasArchived: selected.some(isArchived),
    canGoBack: nav.back.length > 0,
    canGoForward: nav.forward.length > 0,
    canGoUp: nav.prefix !== '',
    inspectorVisible: ui.inspectorVisible,
    filtersVisible: ui.filtersOpen,
    showHidden: settings?.general.showHidden ?? false,
    showDeleted: ui.showDeleted,
    viewMode: ui.viewMode,
  };
}

/** キーボード・メニューの購読と、メニューの状態の同期（メインウィンドウ）。 */
export function useCommands(enabled: boolean) {
  useEffect(() => {
    if (!enabled) return;
    window.addEventListener('keydown', onKeyDown);
    const unlisten = ipc.events.onMenu((a) => handleCommand(a.id));
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      void unlisten.then((f) => f()).catch(() => {});
    };
  }, [enabled]);
}

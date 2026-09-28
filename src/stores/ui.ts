// UI だけの状態（01 §5.4）: 画面、表示モード、並べ替え、選択、インスペクタ、検索語とフィルタ、ダイアログ。

import { create } from 'zustand';
import type {
  DateFilter,
  Entry,
  FileKind,
  RemoteConflict,
  SearchQuery,
  SearchResult,
  SizeFilter,
  Sort,
  StorageClass,
  UploadPlan,
  ViewMode,
} from '@/lib/ipc';

export type View = 'signin' | 'files' | 'dashboard';

export interface Filters {
  kind: FileKind | 'all';
  ext: string;
  size: SizeFilter | 'all';
  date: DateFilter | 'all';
  storageClass: StorageClass | 'all';
}

export const NO_FILTERS: Filters = { kind: 'all', ext: '', size: 'all', date: 'all', storageClass: 'all' };

/** 指定されているフィルタの数。 */
export function filterCount(f: Filters): number {
  return (Object.keys(NO_FILTERS) as (keyof Filters)[]).filter(
    (k) => f[k] !== NO_FILTERS[k] && String(f[k]).trim() !== '',
  ).length;
}

/** 検索語とフィルタから検索条件を作る（04 §10.2）。 */
export function toSearchQuery(text: string, f: Filters, sort: Sort, offset = 0, limit = 1000): SearchQuery {
  const q: SearchQuery = { text: text.trim(), sort, offset, limit };
  if (f.kind !== 'all') q.kind = f.kind;
  const ext = f.ext.trim().replace(/^\./, '').toLowerCase();
  if (ext) q.ext = ext;
  if (f.size !== 'all') q.size = f.size;
  if (f.date !== 'all') q.date = f.date;
  if (f.storageClass !== 'all') q.storageClass = f.storageClass;
  return q;
}

/**
 * 検索結果の次のページの offset（04 §10.2）。フォルダは最初のページの先頭にだけ含まれ、offset・limit は
 * ファイルだけに適用されるため、読み込んだファイルの件数を次の offset にする。すべて読み込んだら `undefined`。
 */
export function nextSearchOffset(pages: SearchResult[]): number | undefined {
  const first = pages[0];
  if (!first) return 0;
  const files = pages.reduce((n, p) => n + p.entries.filter((r) => r.entry.type === 'file').length, 0);
  const folders = first.entries.filter((r) => r.entry.type === 'folder').length;
  // 最後のページが空なら、件数が変わっても続きはない
  if (pages.length > 1 && pages.at(-1)?.entries.length === 0) return undefined;
  return files < first.total - folders ? files : undefined;
}

/** ダイアログの種類とペイロード。 */
export type DialogState =
  | { type: 'newFolder' }
  | { type: 'delete'; items: Entry[] }
  | { type: 'move'; items: Entry[] }
  | { type: 'rename'; item: Entry }
  | { type: 'storageClass'; items: Entry[] }
  | { type: 'restore'; items: Entry[] }
  | {
      type: 'conflict';
      conflicts: RemoteConflict[] | UploadPlan['conflicts'];
      versioned: boolean;
      resolve: ConflictResolver;
    }
  | {
      type: 'deleteVersion';
      key: string;
      versionId: string;
      date: string;
      size: number | null;
      isLatest: boolean;
    }
  | { type: 'addBucket' }
  | { type: 'editConnection'; connectionId: string }
  | { type: 'deleteConnection'; connectionId: string }
  | { type: 'credentials'; credentialId?: string }
  | { type: 'update'; version: string; notes: string | null }
  | { type: 'details'; title: string; lines: string[] };

export type ConflictResolver = (result: Record<string, 'replace' | 'skip' | 'keepBoth'> | null) => void;

export interface Selection {
  keys: string[];
  /** ⇧クリック・⇧↑↓ の起点 */
  anchor: string | null;
}

export interface UiState {
  view: View;
  viewMode: ViewMode;
  sort: Sort;
  selection: Selection;
  inspectorVisible: boolean;
  inspectorTab: 'details' | 'versions';
  query: string;
  filters: Filters;
  filtersOpen: boolean;
  showDeleted: boolean;
  dialog: DialogState | null;
  setView: (view: View) => void;
  setViewMode: (mode: ViewMode) => void;
  toggleSort: (key: Sort['key']) => void;
  setSort: (sort: Sort) => void;
  setInspectorVisible: (visible: boolean) => void;
  setInspectorTab: (tab: 'details' | 'versions') => void;
  setQuery: (query: string) => void;
  setFilters: (filters: Filters) => void;
  clearSearch: () => void;
  setFiltersOpen: (open: boolean) => void;
  setShowDeleted: (show: boolean) => void;
  openDialog: (dialog: DialogState) => void;
  closeDialog: () => void;
  /** クリック（⌘・⇧ を考慮）。`order` は表示中の並び順のキー。 */
  select: (key: string | null, mods: { meta?: boolean; shift?: boolean }, order: string[]) => void;
  selectAll: (order: string[]) => void;
  setSelection: (keys: string[]) => void;
  /** ↑↓（⇧ で範囲の拡張）。 */
  moveSelection: (delta: 1 | -1, extend: boolean, order: string[]) => void;
}

export const useUiStore = create<UiState>((set, get) => ({
  view: 'files',
  viewMode: 'list',
  sort: { key: 'name', dir: 1 },
  selection: { keys: [], anchor: null },
  inspectorVisible: true,
  inspectorTab: 'details',
  query: '',
  filters: NO_FILTERS,
  filtersOpen: false,
  showDeleted: false,
  dialog: null,
  setView: (view) => set({ view, selection: { keys: [], anchor: null } }),
  setViewMode: (viewMode) => set({ viewMode }),
  toggleSort: (key) =>
    set((s) => ({ sort: { key, dir: s.sort.key === key ? (s.sort.dir === 1 ? -1 : 1) : 1 } })),
  setSort: (sort) => set({ sort }),
  setInspectorVisible: (inspectorVisible) => set({ inspectorVisible }),
  setInspectorTab: (inspectorTab) => set({ inspectorTab }),
  setQuery: (query) => set({ query, selection: { keys: [], anchor: null } }),
  setFilters: (filters) => set({ filters, selection: { keys: [], anchor: null } }),
  clearSearch: () => set({ query: '', filters: NO_FILTERS, selection: { keys: [], anchor: null } }),
  setFiltersOpen: (filtersOpen) => set({ filtersOpen }),
  setShowDeleted: (showDeleted) => set({ showDeleted, selection: { keys: [], anchor: null } }),
  openDialog: (dialog) => set({ dialog }),
  closeDialog: () => set({ dialog: null }),
  select: (key, mods, order) => {
    const { selection } = get();
    if (key === null) {
      set({ selection: { keys: [], anchor: null } });
      return;
    }
    if (mods.meta) {
      const has = selection.keys.includes(key);
      set({
        selection: {
          keys: has ? selection.keys.filter((k) => k !== key) : [...selection.keys, key],
          anchor: key,
        },
      });
      return;
    }
    if (mods.shift && selection.anchor) {
      const a = order.indexOf(selection.anchor);
      const b = order.indexOf(key);
      if (a >= 0 && b >= 0) {
        set({
          selection: { keys: order.slice(Math.min(a, b), Math.max(a, b) + 1), anchor: selection.anchor },
        });
        return;
      }
    }
    set({ selection: { keys: [key], anchor: key } });
  },
  selectAll: (order) => set({ selection: { keys: [...order], anchor: order[0] ?? null } }),
  setSelection: (keys) => set({ selection: { keys, anchor: keys[keys.length - 1] ?? null } }),
  moveSelection: (delta, extend, order) => {
    if (order.length === 0) return;
    const { selection } = get();
    const last = selection.keys[selection.keys.length - 1];
    const current = last === undefined ? -1 : order.indexOf(last);
    const next = Math.min(
      order.length - 1,
      Math.max(0, current < 0 ? (delta > 0 ? 0 : order.length - 1) : current + delta),
    );
    const key = order[next];
    if (key === undefined) return;
    if (extend && selection.anchor) {
      const a = order.indexOf(selection.anchor);
      // 範囲の末尾（移動先）を最後に置き、次の移動の基準にする
      const keys = order.slice(Math.min(a, next), Math.max(a, next) + 1);
      set({ selection: { keys: [...keys.filter((k) => k !== key), key], anchor: selection.anchor } });
      return;
    }
    set({ selection: { keys: [key], anchor: key } });
  },
}));

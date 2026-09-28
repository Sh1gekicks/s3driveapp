// ファイル一覧（リスト／アイコン）、ドロップによるアップロード、ステータスバー（03 §5.4）。
// DS: ui_kits/s3-drive/FileList.jsx。行数が多くても軽く動くよう、表示範囲だけを描画する。

import { useVirtualizer } from '@tanstack/react-virtual';
import { ChevronDown, ChevronUp, CircleAlert, Clock, CloudUpload, SearchX } from 'lucide-react';
import * as React from 'react';
import { useWindowFocused } from '@/app/hooks';
import { FileIcon } from '@/components/ds/file-icon';
import { Icon } from '@/components/ds/icon';
import { StorageClassBadge } from '@/components/ds/storage-class-badge';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { ContextMenu } from '@/components/ui/menu';
import { Empty, Skeleton } from '@/components/ui/misc';
import {
  openEntry,
  persistView,
  reload,
  runSelectionAction,
  type SelectionAction,
  upload,
  uploadSelection,
} from '@/features/actions';
import { setVisibleEntries } from '@/features/context';
import { showError } from '@/features/errors';
import { fileKind, KIND_LABEL } from '@/lib/file-kind';
import { formatDate, formatSize } from '@/lib/format';
import { ja } from '@/lib/i18n/ja';
import type { Entry, SortKey } from '@/lib/ipc';
import * as ipc from '@/lib/ipc';
import { requireMock } from '@/lib/ipc/mock-loader';
import { isNative } from '@/lib/platform';
import { cn } from '@/lib/utils';
import { useNavStore } from '@/stores/nav';
import { useUiStore } from '@/stores/ui';
import { blankMenu, itemMenu } from './menus';

const ROW_H = 28;
const TILE_H = 112;
const TILE_MIN_W = 104;
const GRID_GAP = 8;
const GRID_PAD = 16;

const t = ja.list;

export interface FileListProps {
  entries: Entry[];
  /** 検索中は親フォルダのパスを併記する。 */
  parents?: Map<string, string>;
  loading: boolean;
  error: unknown;
  onRetry: () => void;
  searching: boolean;
  narrow: boolean;
  bucket: string;
  status: React.ReactNode;
  /** 一覧の末尾付近まで表示したときに呼ぶ（検索結果の続きの取得。04 §10.2）。 */
  onEndReached?: () => void;
}

/** 末尾からこの行数以内が表示されたら続きを取得する。 */
const END_REACHED_ROWS = 20;

function useElementWidth(ref: React.RefObject<HTMLElement | null>): number {
  const [width, setWidth] = React.useState(0);
  React.useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    setWidth(el.clientWidth);
    const ro = new ResizeObserver(([entry]) => {
      if (entry) setWidth(entry.contentRect.width);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [ref]);
  return width;
}

function RestoreBadge({ entry }: { entry: Entry }) {
  if (entry.type !== 'file') return null;
  switch (entry.restore.state) {
    case 'archived':
      return <Badge variant="warning">{t.restoreNeeded}</Badge>;
    case 'inProgress':
      return (
        <Badge variant="warning" icon={Clock}>
          {t.restoring}
        </Badge>
      );
    case 'restored':
      return <Badge variant="success">{t.restored}</Badge>;
    default:
      return null;
  }
}

function HeadCell({ k, label, align }: { k?: SortKey; label: string; align?: 'right' }) {
  const sort = useUiStore((s) => s.sort);
  const active = k !== undefined && sort.key === k;
  const content = (
    <>
      {label}
      {active ? <Icon icon={sort.dir > 0 ? ChevronUp : ChevronDown} size={12} /> : null}
    </>
  );
  const className = cn(
    'flex h-full items-center gap-1 px-2 font-medium text-muted-foreground',
    align === 'right' && 'justify-end',
    active && 'text-foreground',
  );
  if (!k) return <div className={className}>{content}</div>;
  return (
    <button
      type="button"
      aria-label={active ? `${label}（${sort.dir > 0 ? '昇順' : '降順'}）` : label}
      className={cn(
        className,
        'outline-none hover:text-foreground focus-visible:shadow-[0_0_0_3px_var(--ring-soft)]',
      )}
      onClick={() => {
        useUiStore.getState().toggleSort(k);
        persistView();
      }}
    >
      {content}
    </button>
  );
}

export function FileList({
  entries,
  parents,
  loading,
  error,
  onRetry,
  searching,
  narrow,
  bucket,
  status,
  onEndReached,
}: FileListProps) {
  const viewMode = useUiStore((s) => s.viewMode);
  const selectionKeys = useUiStore((s) => s.selection.keys);
  const prefix = useNavStore((s) => s.prefix);
  const focused = useWindowFocused();
  const scrollRef = React.useRef<HTMLDivElement>(null);
  const width = useElementWidth(scrollRef);
  const [drop, setDrop] = React.useState<{ key: string | null } | null>(null);
  const selected = React.useMemo(() => new Set(selectionKeys), [selectionKeys]);
  const order = React.useMemo(() => entries.map((e) => e.key), [entries]);

  React.useEffect(() => {
    setVisibleEntries(entries);
  }, [entries]);

  const cols = narrow ? 'minmax(140px,1fr) 124px 72px 112px' : 'minmax(200px,1fr) 140px 80px 120px 120px';
  const perRow = Math.max(1, Math.floor((width - GRID_PAD * 2 + GRID_GAP) / (TILE_MIN_W + GRID_GAP)));
  const rowCount = viewMode === 'list' ? entries.length : Math.ceil(entries.length / perRow);

  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => (viewMode === 'list' ? ROW_H : TILE_H),
    overscan: 12,
    // 最初の描画（と jsdom）で寸法が 0 でも行を描けるようにする
    initialRect: { width: 1000, height: 800 },
    paddingStart: viewMode === 'list' ? 4 : GRID_PAD,
    paddingEnd: viewMode === 'list' ? 4 : GRID_PAD,
  });

  const virtualItems = virtualizer.getVirtualItems();
  const lastVisible = virtualItems[virtualItems.length - 1]?.index ?? -1;
  React.useEffect(() => {
    if (onEndReached && rowCount > 0 && lastVisible >= rowCount - END_REACHED_ROWS) onEndReached();
  }, [onEndReached, lastVisible, rowCount]);

  // 空白部分のクリック・右クリックで選択を解除する（キーボードでは Esc）。項目の上では項目側の処理に任せる
  React.useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const clear = (e: MouseEvent) => {
      if (e.type === 'mousedown' && e.button !== 0) return;
      if (e.target instanceof Element && e.target.closest('[aria-selected]')) return;
      useUiStore.getState().select(null, {}, []);
    };
    el.addEventListener('mousedown', clear);
    el.addEventListener('contextmenu', clear);
    return () => {
      el.removeEventListener('mousedown', clear);
      el.removeEventListener('contextmenu', clear);
    };
  }, []);

  // キーボードで選択を動かしたら、選択した行が見えるようにする
  const lastSelected = selectionKeys[selectionKeys.length - 1];
  React.useEffect(() => {
    if (!lastSelected) return;
    const index = order.indexOf(lastSelected);
    if (index < 0) return;
    virtualizer.scrollToIndex(viewMode === 'list' ? index : Math.floor(index / perRow), { align: 'auto' });
  }, [lastSelected, order, viewMode, perRow, virtualizer]);

  // ---- 選択・メニュー ----
  const onItemMouseDown = (e: React.MouseEvent, entry: Entry) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    useUiStore.getState().select(entry.key, { meta: e.metaKey || e.ctrlKey, shift: e.shiftKey }, order);
  };
  const onItemContextMenu = (entry: Entry) => {
    if (!useUiStore.getState().selection.keys.includes(entry.key)) {
      useUiStore.getState().select(entry.key, {}, order);
    }
  };
  const selectedEntries = entries.filter((e) => selected.has(e.key));
  const menuItems = selectedEntries.length > 0 ? itemMenu(selectedEntries) : searching ? [] : blankMenu();
  const onMenu = (id: string) => {
    switch (id) {
      case 'newFolder':
        useUiStore.getState().openDialog({ type: 'newFolder' });
        return;
      case 'upload':
        void upload();
        return;
      case 'uploadFolder':
        void upload(true);
        return;
      case 'reload':
        reload();
        return;
      default:
        runSelectionAction(id as SelectionAction, selectedEntries);
    }
  };

  // ---- ドロップによるアップロード ----
  const dropTargetAt = React.useCallback((x: number, y: number): string | null => {
    const el = document.elementFromPoint(x, y)?.closest<HTMLElement>('[data-drop-folder]');
    return el?.dataset.dropFolder ?? null;
  }, []);

  React.useEffect(() => {
    if (!isNative() || searching) return;
    const unlisten = [
      ipc.events.onDragEnter((p) =>
        setDrop({ key: p.position ? dropTargetAt(p.position.x, p.position.y) : null }),
      ),
      ipc.events.onDragOver((p) =>
        setDrop({ key: p.position ? dropTargetAt(p.position.x, p.position.y) : null }),
      ),
      ipc.events.onDragLeave(() => setDrop(null)),
      ipc.events.onDrop((p) => {
        setDrop(null);
        if (useUiStore.getState().dialog) return;
        const target = dropTargetAt(p.position.x, p.position.y);
        void uploadSelection(p.selectionId, target ?? useNavStore.getState().prefix);
      }),
    ];
    return () => {
      for (const u of unlisten) void u.then((f) => f());
    };
  }, [searching, dropTargetAt]);

  // モック（ブラウザ）では HTML のドラッグ＆ドロップを使う
  const htmlDnd =
    isNative() || searching
      ? {}
      : {
          onDragOver: (e: React.DragEvent) => {
            if (!e.dataTransfer.types.includes('Files')) return;
            e.preventDefault();
            setDrop({ key: dropTargetAt(e.clientX, e.clientY) });
          },
          onDragLeave: (e: React.DragEvent) => {
            if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setDrop(null);
          },
          onDrop: (e: React.DragEvent) => {
            e.preventDefault();
            const target = dropTargetAt(e.clientX, e.clientY) ?? prefix;
            setDrop(null);
            const files = Array.from(e.dataTransfer.files);
            if (files.length === 0) return;
            requireMock()
              .then(({ registerBrowserFiles }) => registerBrowserFiles(files))
              .then((sel) => uploadSelection(sel.selectionId, target))
              .catch((err) => showError(err, ja.verbs.upload));
          },
        };

  const dropLabel = t.dropHere(`${bucket}/${drop?.key ?? prefix}`);
  const selectedClass = focused
    ? 'bg-row-selected text-row-selected-foreground'
    : 'bg-row-selected-inactive text-foreground';

  // ---- 本体 ----
  let body: React.ReactNode;
  if (loading && entries.length === 0) {
    body = (
      <div className="flex flex-col gap-1 p-2" aria-busy="true">
        {Array.from({ length: 8 }, (_, i) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: スケルトンは並びが変わらない
          <Skeleton key={i} className="h-5" style={{ width: `${70 - i * 5}%` }} />
        ))}
      </div>
    );
  } else if (error && entries.length === 0) {
    body = (
      <Empty
        icon={CircleAlert}
        action={
          <Button variant="outline" size="sm" onClick={onRetry}>
            {ja.common.retry}
          </Button>
        }
      >
        {ipc.toAppError(error).message || t.loadFailed}
      </Empty>
    );
  } else if (entries.length === 0) {
    body = searching ? (
      <Empty icon={SearchX}>{t.noResults}</Empty>
    ) : (
      <Empty icon={CloudUpload}>{t.emptyFolder}</Empty>
    );
  } else if (viewMode === 'list') {
    body = (
      <div className="text-base">
        <div
          className="sticky top-0 z-1 grid h-7 items-center bg-background px-2 text-sm hairline-b"
          style={{ gridTemplateColumns: cols }}
        >
          <HeadCell k="name" label={t.name} />
          <HeadCell k="modified" label={t.modified} />
          <HeadCell k="size" label={t.size} align="right" />
          {narrow ? null : <HeadCell label={t.kind} />}
          <HeadCell k="storageClass" label={t.storageClass} />
        </div>
        <div
          role="listbox"
          aria-label={bucket}
          aria-multiselectable="true"
          className="relative px-2"
          style={{ height: virtualizer.getTotalSize() }}
        >
          {virtualizer.getVirtualItems().map((v) => {
            const entry = entries[v.index];
            if (!entry) return null;
            const isSel = selected.has(entry.key);
            const folder = entry.type === 'folder';
            const sub = isSel && focused ? 'opacity-85' : 'text-muted-foreground';
            return (
              <div
                key={entry.key}
                role="option"
                tabIndex={-1}
                aria-selected={isSel}
                aria-setsize={entries.length}
                aria-posinset={v.index + 1}
                data-drop-folder={folder && !entry.deleted ? entry.key : undefined}
                onMouseDown={(e) => onItemMouseDown(e, entry)}
                onDoubleClick={() => openEntry(entry)}
                onContextMenu={() => onItemContextMenu(entry)}
                className={cn(
                  'absolute right-2 left-2 grid items-center rounded-md',
                  isSel
                    ? selectedClass
                    : v.index % 2
                      ? 'bg-row-stripe hover:bg-row-hover'
                      : 'hover:bg-row-hover',
                  entry.deleted && 'opacity-55',
                  drop?.key === entry.key && 'shadow-[inset_0_0_0_2px_var(--primary)]',
                )}
                style={{
                  gridTemplateColumns: cols,
                  height: ROW_H,
                  transform: `translateY(${v.start}px)`,
                  top: 0,
                }}
              >
                <div className="flex min-w-0 items-center gap-2 px-2">
                  <FileIcon name={entry.name} folder={folder} />
                  <span className="truncate">{entry.name}</span>
                  {entry.deleted ? <Badge variant="destructive">{t.deleteMarker}</Badge> : null}
                  {parents?.get(entry.key) ? (
                    <span className={cn('truncate text-sm', sub)}>{parents.get(entry.key)}</span>
                  ) : null}
                </div>
                <div className={cn('px-2 text-sm tabular-nums', sub)}>{formatDate(entry.lastModified)}</div>
                <div className={cn('px-2 text-right text-sm tabular-nums', sub)}>
                  {entry.type === 'file' ? formatSize(entry.size) : ja.common.none}
                </div>
                {narrow ? null : (
                  <div className={cn('truncate px-2 text-sm', sub)}>
                    {KIND_LABEL[fileKind(entry.name, folder)]}
                  </div>
                )}
                <div className="flex min-w-0 items-center gap-1 px-2">
                  {entry.type === 'file' ? (
                    <>
                      <StorageClassBadge
                        value={entry.storageClass}
                        plain
                        className={isSel && focused ? 'text-inherit' : undefined}
                      />
                      <RestoreBadge entry={entry} />
                    </>
                  ) : (
                    <span className={sub}>{ja.common.none}</span>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    );
  } else {
    body = (
      <div
        role="listbox"
        aria-label={bucket}
        aria-multiselectable="true"
        className="relative"
        style={{ height: virtualizer.getTotalSize() }}
      >
        {virtualizer.getVirtualItems().map((v) => (
          <div
            key={v.key}
            className="absolute right-4 left-4 grid"
            style={{
              gridTemplateColumns: `repeat(${perRow}, minmax(${TILE_MIN_W}px, 1fr))`,
              gap: GRID_GAP,
              height: TILE_H,
              transform: `translateY(${v.start}px)`,
              top: 0,
            }}
          >
            {entries.slice(v.index * perRow, v.index * perRow + perRow).map((entry) => {
              const isSel = selected.has(entry.key);
              const folder = entry.type === 'folder';
              return (
                <div
                  key={entry.key}
                  role="option"
                  tabIndex={-1}
                  aria-selected={isSel}
                  data-drop-folder={folder && !entry.deleted ? entry.key : undefined}
                  onMouseDown={(e) => onItemMouseDown(e, entry)}
                  onDoubleClick={() => openEntry(entry)}
                  onContextMenu={() => onItemContextMenu(entry)}
                  className={cn(
                    'flex min-w-0 flex-col items-center gap-1.5 p-1.5',
                    entry.deleted && 'opacity-55',
                  )}
                >
                  <div
                    className={cn(
                      'grid h-16 w-18 place-items-center rounded-lg',
                      isSel && 'bg-row-selected-inactive',
                      drop?.key === entry.key && 'shadow-[inset_0_0_0_2px_var(--primary)]',
                    )}
                  >
                    <FileIcon name={entry.name} folder={folder} size={44} />
                  </div>
                  <span
                    className={cn(
                      'line-clamp-2 max-w-full rounded-xs px-1.25 py-px text-center text-sm break-all',
                      isSel && selectedClass,
                    )}
                    title={entry.name}
                  >
                    {entry.name}
                  </span>
                </div>
              );
            })}
          </div>
        ))}
      </div>
    );
  }

  return (
    <div className="relative flex min-h-0 flex-1 flex-col" {...htmlDnd}>
      <ContextMenu items={menuItems} onSelect={onMenu} className="flex min-h-0 flex-1 flex-col">
        <div ref={scrollRef} className="min-h-0 flex-1 overflow-auto">
          {body}
        </div>
      </ContextMenu>
      <div
        role="status"
        className="flex h-6.5 shrink-0 items-center justify-center text-xs text-muted-foreground tabular-nums hairline-t"
      >
        {status}
      </div>
      {drop ? (
        <div className="pointer-events-none absolute inset-2 grid place-items-center rounded-xl border-2 border-dashed border-primary bg-[color-mix(in_oklch,var(--primary)_8%,transparent)]">
          <div className="flex flex-col items-center gap-2 text-base font-semibold text-primary">
            <Icon icon={CloudUpload} size={32} />
            {dropLabel}
          </div>
        </div>
      ) : null}
    </div>
  );
}

// ツールバーとフィルタバー（03 §5.2〜5.3）。DS: ui_kits/s3-drive/Toolbar.jsx。

import {
  ChevronLeft,
  ChevronRight,
  FolderPlus,
  LayoutGrid,
  List,
  PanelRight,
  RefreshCw,
  Search,
  SlidersHorizontal,
  Upload,
} from 'lucide-react';
import { useEffect, useRef } from 'react';
import { DragSpacer } from '@/components/ds/app-shell';
import { Icon } from '@/components/ds/icon';
import { Breadcrumbs } from '@/components/ui/breadcrumbs';
import { Button } from '@/components/ui/button';
import { IconButton } from '@/components/ui/icon-button';
import { Input } from '@/components/ui/input';
import { Separator } from '@/components/ui/misc';
import { NativeSelect } from '@/components/ui/native-select';
import { SegmentedControl } from '@/components/ui/segmented-control';
import { persistView, upload } from '@/features/actions';
import { FILE_KINDS, KIND_LABEL } from '@/lib/file-kind';
import { formatRelative } from '@/lib/format';
import { ja } from '@/lib/i18n/ja';
import type { Connection, IndexStatus, ViewMode } from '@/lib/ipc';
import { SELECTABLE_CLASSES, STORAGE_CLASSES } from '@/lib/storage-class';
import { useNavStore } from '@/stores/nav';
import { type Filters, filterCount, NO_FILTERS, useUiStore } from '@/stores/ui';

const t = ja.toolbar;

/** 検索欄へのフォーカス（⌘F）。 */
let focusSearch: (() => void) | null = null;
export function focusSearchInput() {
  focusSearch?.();
}

export function BrowserToolbar({ connection, narrow }: { connection: Connection; narrow: boolean }) {
  const nav = useNavStore();
  const query = useUiStore((s) => s.query);
  const filters = useUiStore((s) => s.filters);
  const filtersOpen = useUiStore((s) => s.filtersOpen);
  const viewMode = useUiStore((s) => s.viewMode);
  const inspectorVisible = useUiStore((s) => s.inspectorVisible);
  const inputRef = useRef<HTMLInputElement>(null);
  const count = filterCount(filters);

  useEffect(() => {
    focusSearch = () => {
      inputRef.current?.focus();
      inputRef.current?.select();
    };
    return () => {
      focusSearch = null;
    };
  }, []);

  const parts = nav.prefix.split('/').filter(Boolean);
  const crumbs = [{ label: connection.bucket }, ...parts.map((label) => ({ label }))];
  const onCrumb = (i: number) => {
    const prefix = i === 0 ? '' : `${parts.slice(0, i).join('/')}/`;
    const ui = useUiStore.getState();
    if (ui.query || filterCount(ui.filters)) ui.clearSearch();
    if (prefix !== nav.prefix) nav.navigate(prefix);
  };

  return (
    <>
      <div className="flex gap-0.5">
        <IconButton
          icon={ChevronLeft}
          label={t.back}
          shortcut="⌘["
          disabled={nav.back.length === 0}
          onClick={nav.goBack}
        />
        <IconButton
          icon={ChevronRight}
          label={t.forward}
          shortcut="⌘]"
          disabled={nav.forward.length === 0}
          onClick={nav.goForward}
        />
      </div>
      <Breadcrumbs items={crumbs} onNavigate={onCrumb} className="min-w-15 flex-[0_1_auto] overflow-hidden" />
      <DragSpacer />
      <div className="flex min-w-0 flex-[0_1_auto] items-center gap-2">
        <Input
          ref={inputRef}
          icon={Search}
          type="search"
          placeholder={t.search}
          aria-label={t.search}
          value={query}
          onChange={(e) => useUiStore.getState().setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Escape') {
              e.preventDefault();
              useUiStore.getState().setQuery('');
              e.currentTarget.blur();
            }
          }}
          wrapperClassName="w-[180px] min-w-[110px] flex-[0_1_180px]"
        />
        <IconButton
          icon={SlidersHorizontal}
          label={t.filter(count)}
          shortcut="⌥⌘F"
          active={filtersOpen || count > 0}
          onClick={() => useUiStore.getState().setFiltersOpen(!filtersOpen)}
        />
        <SegmentedControl<ViewMode>
          aria-label="表示"
          value={viewMode}
          onChange={(mode) => {
            useUiStore.getState().setViewMode(mode);
            persistView();
          }}
          options={[
            { value: 'list', label: t.list, icon: List, iconOnly: true, shortcut: '⌘2' },
            { value: 'grid', label: t.grid, icon: LayoutGrid, iconOnly: true, shortcut: '⌘1' },
          ]}
        />
        <Separator vertical />
        <IconButton
          icon={FolderPlus}
          label={t.newFolder}
          shortcut="⇧⌘N"
          onClick={() => useUiStore.getState().openDialog({ type: 'newFolder' })}
        />
        {narrow ? (
          <IconButton
            icon={Upload}
            label={t.upload}
            shortcut="⌘U"
            variant="default"
            onClick={() => void upload()}
          />
        ) : (
          <Button onClick={() => void upload()} title={`${t.upload} ⌘U`}>
            <Icon icon={Upload} size={16} />
            {t.upload}
          </Button>
        )}
        <IconButton
          icon={PanelRight}
          label={inspectorVisible ? t.hideInspector : t.showInspector}
          shortcut="⌥⌘I"
          active={inspectorVisible}
          onClick={() => {
            useUiStore.getState().setInspectorVisible(!inspectorVisible);
            persistView();
          }}
        />
      </div>
    </>
  );
}

export function FilterBar({ total }: { total: number | null }) {
  const filters = useUiStore((s) => s.filters);
  const set = <K extends keyof Filters>(key: K, value: Filters[K]) =>
    useUiStore.getState().setFilters({ ...useUiStore.getState().filters, [key]: value });
  const f = ja.filter;
  return (
    <div className="flex flex-wrap items-center gap-2 bg-muted px-3.5 py-2 hairline-b">
      <span className="text-sm font-medium text-muted-foreground">{f.label}</span>
      <NativeSelect
        size="sm"
        aria-label="種類"
        wrapperClassName="w-[130px]"
        value={filters.kind}
        onChange={(e) => set('kind', e.target.value as Filters['kind'])}
        options={[
          { value: 'all', label: f.allKinds },
          ...FILE_KINDS.map((k) => ({ value: k, label: KIND_LABEL[k] })),
        ]}
      />
      <Input
        size="sm"
        aria-label="拡張子"
        placeholder={f.extension}
        wrapperClassName="w-[130px]"
        value={filters.ext}
        onChange={(e) => set('ext', e.target.value)}
      />
      <NativeSelect
        size="sm"
        aria-label="サイズ"
        wrapperClassName="w-[130px]"
        value={filters.size}
        onChange={(e) => set('size', e.target.value as Filters['size'])}
        options={[
          { value: 'all', label: f.allSizes },
          { value: 'lt1', label: f.lt1 },
          { value: '1to100', label: f.oneTo100 },
          { value: 'gt100', label: f.gt100 },
        ]}
      />
      <NativeSelect
        size="sm"
        aria-label="期間"
        wrapperClassName="w-[120px]"
        value={filters.date}
        onChange={(e) => set('date', e.target.value as Filters['date'])}
        options={[
          { value: 'all', label: f.allDates },
          { value: '7d', label: f.days7 },
          { value: '30d', label: f.days30 },
          { value: 'year', label: f.year },
        ]}
      />
      <NativeSelect
        size="sm"
        aria-label="ストレージクラス"
        wrapperClassName="w-[130px]"
        value={filters.storageClass}
        onChange={(e) => set('storageClass', e.target.value as Filters['storageClass'])}
        options={[
          { value: 'all', label: f.allClasses },
          ...SELECTABLE_CLASSES.map((c) => ({ value: c, label: STORAGE_CLASSES[c].short })),
        ]}
      />
      <Button
        variant="link"
        size="sm"
        onClick={() => {
          useUiStore.getState().setFilters(NO_FILTERS);
          useUiStore.getState().setQuery('');
        }}
      >
        {ja.common.clear}
      </Button>
      <span className="ml-auto text-sm text-muted-foreground tabular-nums">
        {total == null ? '' : f.count(total)}
      </span>
    </div>
  );
}

/** 検索中の 1 行（「{バケット} 全体を検索中 · {件数} 件 · インデックス {n} 分前に更新（更新）」）。 */
export function SearchStatus({
  bucket,
  total,
  index,
  onRefresh,
}: {
  bucket: string;
  total: number | null;
  index: IndexStatus | null;
  onRefresh: () => void;
}) {
  const f = ja.filter;
  return (
    <div className="flex h-7 shrink-0 items-center gap-1.5 px-3.5 text-sm text-muted-foreground hairline-b">
      <Icon icon={Search} size={12} />
      <span className="truncate">{f.searching(bucket, total ?? 0)}</span>
      {index ? (
        <>
          <span>·</span>
          <span className="truncate">
            {index.state === 'building'
              ? f.indexBuilding(index.progress ?? 0)
              : index.lastScanAt
                ? f.indexAge(formatRelative(index.lastScanAt))
                : ''}
          </span>
          {index.state !== 'building' ? (
            <Button variant="link" size="sm" onClick={onRefresh} className="gap-1">
              <Icon icon={RefreshCw} size={11} />
              {ja.common.update}
            </Button>
          ) : null}
        </>
      ) : null}
    </div>
  );
}

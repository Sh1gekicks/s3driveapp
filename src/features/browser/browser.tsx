// SCR-02 ファイルブラウザの本体（フィルタバー、検索の状態、一覧、ステータスバー）。

import { useQueryClient } from '@tanstack/react-query';
import { useCallback, useMemo } from 'react';
import {
  useBucketInfo,
  useDebounced,
  useIndexStatus,
  useListing,
  useSearch,
  useSettings,
} from '@/app/queries';
import { qk } from '@/app/query-keys';
import { showError } from '@/features/errors';
import { formatSize } from '@/lib/format';
import { ja } from '@/lib/i18n/ja';
import type { Connection, Entry } from '@/lib/ipc';
import * as ipc from '@/lib/ipc';
import { sortEntries } from '@/lib/sort';
import { useNavStore } from '@/stores/nav';
import { filterCount, toSearchQuery, useUiStore } from '@/stores/ui';
import { FileList } from './file-list';
import { FilterBar, SearchStatus } from './toolbar';

/** 1 つのフォルダの項目がこれを超えたら、ステータスバーで検索を勧める（04 §3.5）。 */
const MANY_ITEMS = 100_000;

export function Browser({ connection, narrow }: { connection: Connection; narrow: boolean }) {
  const queryClient = useQueryClient();
  const prefix = useNavStore((s) => s.prefix);
  const query = useUiStore((s) => s.query);
  const filters = useUiStore((s) => s.filters);
  const filtersOpen = useUiStore((s) => s.filtersOpen);
  const sort = useUiStore((s) => s.sort);
  const showDeleted = useUiStore((s) => s.showDeleted);
  const selectionKeys = useUiStore((s) => s.selection.keys);
  const settings = useSettings();
  const showHidden = settings.data?.general.showHidden ?? false;
  // バージョニングの有無でダイアログの文言と削除の方法が変わるため、インスペクタを閉じていても取得しておく。
  // リージョンが違っていた場合は、ここで修正される（01 §6.1）
  useBucketInfo(connection.id);

  const debouncedQuery = useDebounced(query, 200);
  const searching = debouncedQuery.trim() !== '' || filterCount(filters) > 0;
  const searchQuery = useMemo(
    () => (searching ? toSearchQuery(debouncedQuery, filters, sort) : null),
    [searching, debouncedQuery, filters, sort],
  );
  const search = useSearch(connection.id, searchQuery);
  const index = useIndexStatus(connection.id, searching);
  const listing = useListing(connection.id, prefix, { showHidden, includeDeleted: showDeleted }, !searching);

  const sorted = useMemo(() => sortEntries(listing.entries, sort), [listing.entries, sort]);
  const { entries, parents } = useMemo(() => {
    if (!searching) return { entries: sorted, parents: undefined };
    const list: Entry[] = [];
    const map = new Map<string, string>();
    for (const r of search.results) {
      list.push(r.entry);
      map.set(r.entry.key, r.parent || '/');
    }
    return { entries: list, parents: map };
  }, [searching, sorted, search.results]);

  const total = searching ? search.total : entries.length;
  const indexStatus = search.index ?? index.data ?? null;
  const { hasNextPage, isFetchingNextPage, fetchNextPage } = search;
  const loadMore = useCallback(() => {
    if (hasNextPage && !isFetchingNextPage) void fetchNextPage();
  }, [hasNextPage, isFetchingNextPage, fetchNextPage]);

  const rebuild = () => {
    ipc.search
      .rebuild(connection.id, (e) => {
        if (e.event === 'finished') {
          void queryClient.invalidateQueries({ queryKey: qk.search(connection.id) });
          void queryClient.invalidateQueries({ queryKey: qk.indexStatus(connection.id) });
        }
        if (e.event === 'failed') showError(e.data.error, '検索インデックスを更新');
      })
      .then(() => queryClient.invalidateQueries({ queryKey: qk.indexStatus(connection.id) }))
      .catch((e) => showError(e, '検索インデックスを更新'));
  };

  // ステータスバー
  const selectedSet = new Set(selectionKeys);
  const selected = entries.filter((e) => selectedSet.has(e.key));
  const selectedBytes = selected.reduce((s, e) => s + (e.type === 'file' ? e.size : 0), 0);
  const parts = [ja.list.items(total ?? entries.length)];
  if (selected.length > 0) {
    parts.push(
      ja.list.selected(
        selected.length,
        selected.some((e) => e.type === 'file') ? formatSize(selectedBytes) : '',
      ),
    );
  } else {
    parts.push(connection.regionShort);
  }
  let status = parts.join(' · ');
  if (!searching && listing.hasNextPage) status = ja.list.loadingMore(entries.length);
  else if (!searching && entries.length >= MANY_ITEMS) status = `${status} · ${ja.list.tooMany}`;

  return (
    <>
      {filtersOpen ? <FilterBar total={searching ? total : null} /> : null}
      {searching ? (
        <SearchStatus bucket={connection.bucket} total={total} index={indexStatus} onRefresh={rebuild} />
      ) : null}
      <FileList
        entries={entries}
        parents={parents}
        loading={searching ? search.isPending : listing.isPending}
        error={searching ? search.error : listing.error}
        onRetry={() => void (searching ? search.refetch() : listing.refetch())}
        searching={searching}
        narrow={narrow}
        bucket={connection.bucket}
        status={status}
        onEndReached={searching ? loadMore : undefined}
      />
    </>
  );
}

// サーバー状態のクエリ（01 §5.3）。

import { keepPreviousData, useInfiniteQuery, useQuery } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import type { Entry, ListOptions, SearchQuery } from '@/lib/ipc';
import * as ipc from '@/lib/ipc';
import { qk, staleTime } from './query-keys';

export function useSession() {
  return useQuery({
    queryKey: qk.session,
    queryFn: () => ipc.auth.restore(),
    staleTime: Number.POSITIVE_INFINITY,
    refetchOnWindowFocus: false,
  });
}

export function useSettings() {
  return useQuery({
    queryKey: qk.settings,
    queryFn: () => ipc.app.settings(),
    staleTime: Number.POSITIVE_INFINITY,
    refetchOnWindowFocus: false,
  });
}

export function useConnections(enabled = true) {
  return useQuery({
    queryKey: qk.connections,
    queryFn: () => ipc.connections.list(),
    staleTime: Number.POSITIVE_INFINITY,
    refetchOnWindowFocus: false,
    enabled,
  });
}

export function useCredentials(enabled = true) {
  return useQuery({
    queryKey: qk.credentials,
    queryFn: () => ipc.connections.credentials(),
    enabled,
  });
}

export function useBucketInfo(connId: string | null) {
  return useQuery({
    queryKey: qk.bucket(connId ?? ''),
    queryFn: () => ipc.connections.bucketInfo(connId as string),
    staleTime: staleTime.bucket,
    enabled: Boolean(connId),
    refetchOnWindowFocus: false,
  });
}

/** フォルダの一覧。1 ページ目を受け取った時点で描画し、残りのページは順に取得して追加する（04 §3.1）。 */
export function useListing(connId: string | null, prefix: string, opts: ListOptions, enabled = true) {
  const query = useInfiniteQuery({
    queryKey: qk.objects(connId ?? '', prefix, opts),
    queryFn: ({ pageParam }) => ipc.objects.listPage(connId as string, prefix, pageParam, opts),
    initialPageParam: null as string | null,
    getNextPageParam: (last) => last.nextToken,
    staleTime: staleTime.objects,
    enabled: Boolean(connId) && enabled,
  });
  const { hasNextPage, isFetchingNextPage, fetchNextPage, isError } = query;
  useEffect(() => {
    if (hasNextPage && !isFetchingNextPage && !isError) void fetchNextPage();
  }, [hasNextPage, isFetchingNextPage, isError, fetchNextPage]);
  const entries = useMemo<Entry[]>(() => query.data?.pages.flatMap((p) => p.entries) ?? [], [query.data]);
  return { ...query, entries };
}

/** 値の変化を `ms` だけ遅らせる（矢印キーで選択を素早く動かした場合に備える。04 §9.1）。 */
export function useDebounced<T>(value: T, ms: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setDebounced(value), ms);
    return () => clearTimeout(t);
  }, [value, ms]);
  return debounced;
}

export function useObjectDetail(connId: string | null, key: string | null, versionId?: string | null) {
  const debouncedKey = useDebounced(key, 150);
  return useQuery({
    queryKey: qk.object(connId ?? '', debouncedKey ?? '', versionId),
    queryFn: () => ipc.objects.head(connId as string, debouncedKey as string, versionId ?? undefined),
    staleTime: staleTime.object,
    enabled: Boolean(connId && debouncedKey && debouncedKey === key),
  });
}

export function useVersions(connId: string | null, key: string | null, enabled: boolean) {
  const debouncedKey = useDebounced(key, 150);
  return useInfiniteQuery({
    queryKey: qk.versions(connId ?? '', debouncedKey ?? ''),
    queryFn: ({ pageParam }) => ipc.versions.list(connId as string, debouncedKey as string, pageParam),
    initialPageParam: null as string | null,
    getNextPageParam: (last) => last.nextCursor,
    staleTime: staleTime.versions,
    enabled: Boolean(connId && debouncedKey && debouncedKey === key && enabled),
  });
}

export function useFolderSummary(connId: string | null, prefix: string, enabled = true) {
  return useQuery({
    queryKey: qk.folderSummary(connId ?? '', prefix),
    queryFn: () => ipc.objects.folderSummary(connId as string, prefix),
    staleTime: staleTime.folderSummary,
    enabled: Boolean(connId) && enabled,
  });
}

export function useFolderChildren(connId: string, prefix: string, enabled: boolean) {
  return useQuery({
    queryKey: qk.folderChildren(connId, prefix),
    queryFn: () => ipc.objects.folderChildren(connId, prefix),
    enabled,
  });
}

export function useSearch(connId: string | null, query: SearchQuery | null) {
  return useQuery({
    queryKey: qk.search(connId ?? '', query ?? undefined),
    queryFn: () => ipc.search.query(connId as string, query as SearchQuery),
    staleTime: staleTime.search,
    enabled: Boolean(connId && query),
    placeholderData: keepPreviousData,
  });
}

export function useIndexStatus(connId: string | null, enabled = true) {
  return useQuery({
    queryKey: qk.indexStatus(connId ?? ''),
    queryFn: () => ipc.search.status(connId as string),
    enabled: Boolean(connId) && enabled,
  });
}

export function useStorageMetrics(connId: string | null) {
  return useQuery({
    queryKey: qk.storageMetrics(connId ?? ''),
    queryFn: () => ipc.metrics.storage(connId as string),
    staleTime: staleTime.storageMetrics,
    enabled: Boolean(connId),
    refetchOnWindowFocus: false,
  });
}

/** 保存済みのコスト。Cost Explorer には問い合わせない（04 §13.4）。 */
export function useCost(connId: string | null) {
  return useQuery({
    queryKey: qk.cost(connId ?? ''),
    queryFn: () => ipc.metrics.cost(connId as string),
    staleTime: staleTime.cost,
    enabled: Boolean(connId),
    refetchOnWindowFocus: false,
  });
}

export function usePricing(region: string | null) {
  return useQuery({
    queryKey: qk.pricing(region ?? ''),
    queryFn: () => ipc.metrics.pricing(region as string),
    staleTime: staleTime.pricing,
    enabled: Boolean(region),
    refetchOnWindowFocus: false,
  });
}

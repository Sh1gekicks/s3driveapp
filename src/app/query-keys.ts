// TanStack Query のクエリキーと staleTime（01 §5.3）。

import type { ListOptions, SearchQuery } from '@/lib/ipc';

export const qk = {
  session: ['session'] as const,
  connections: ['connections'] as const,
  credentials: ['credentials'] as const,
  bucket: (connId: string) => ['bucket', connId] as const,
  objects: (connId: string, prefix: string, opts?: ListOptions) =>
    opts ? (['objects', connId, prefix, opts] as const) : (['objects', connId, prefix] as const),
  objectsAll: (connId: string) => ['objects', connId] as const,
  object: (connId: string, key: string, versionId?: string | null) =>
    ['object', connId, key, versionId ?? null] as const,
  objectAll: (connId: string) => ['object', connId] as const,
  versions: (connId: string, key: string) => ['versions', connId, key] as const,
  folderSummary: (connId: string, prefix: string) => ['folderSummary', connId, prefix] as const,
  folderChildren: (connId: string, prefix: string) => ['folderChildren', connId, prefix] as const,
  search: (connId: string, query?: SearchQuery) =>
    query ? (['search', connId, query] as const) : (['search', connId] as const),
  indexStatus: (connId: string) => ['indexStatus', connId] as const,
  storageMetrics: (connId: string) => ['storageMetrics', connId] as const,
  cost: (connId: string) => ['cost', connId] as const,
  pricing: (region: string) => ['pricing', region] as const,
  settings: ['settings'] as const,
};

const MINUTE = 60_000;

export const staleTime = {
  bucket: 10 * MINUTE,
  objects: 30_000,
  object: 60_000,
  versions: 30_000,
  folderSummary: 5 * MINUTE,
  search: 0,
  storageMetrics: 60 * MINUTE,
  /** コストは手動の「更新」だけで取り直す（04 §13.4） */
  cost: Number.POSITIVE_INFINITY,
  pricing: 7 * 24 * 60 * MINUTE,
};

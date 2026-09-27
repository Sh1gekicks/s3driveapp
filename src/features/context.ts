// 表示中の一覧（フォルダの一覧または検索結果）。選択のキーから項目を引くために使う。

import { useMemo } from 'react';
import { create } from 'zustand';
import type { Entry } from '@/lib/ipc';
import { useUiStore } from '@/stores/ui';

interface VisibleState {
  entries: Entry[];
  byKey: Map<string, Entry>;
}

export const useVisibleStore = create<VisibleState>(() => ({ entries: [], byKey: new Map() }));

/** FileList が表示のたびに登録する。 */
export function setVisibleEntries(entries: Entry[]) {
  useVisibleStore.setState({ entries, byKey: new Map(entries.map((e) => [e.key, e])) });
}

export function visibleEntries(): Entry[] {
  return useVisibleStore.getState().entries;
}

export function visibleOrder(): string[] {
  return visibleEntries().map((e) => e.key);
}

function pick(keys: string[], byKey: Map<string, Entry>): Entry[] {
  return keys.map((k) => byKey.get(k)).filter((e): e is Entry => e !== undefined);
}

/** 選択している項目（表示中のものだけ）。 */
export function selectedEntries(): Entry[] {
  return pick(useUiStore.getState().selection.keys, useVisibleStore.getState().byKey);
}

export function useSelectedEntries(): Entry[] {
  const keys = useUiStore((s) => s.selection.keys);
  const byKey = useVisibleStore((s) => s.byKey);
  return useMemo(() => pick(keys, byKey), [keys, byKey]);
}

export function isArchived(e: Entry): boolean {
  return e.type === 'file' && e.restore.state === 'archived';
}

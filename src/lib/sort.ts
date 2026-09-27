// 一覧の並べ替え（03 §5.4）。フォルダを常に先頭にし、その中で選択した列の順に並べる。

import type { Entry, Sort, StorageClass } from './ipc';
import { SELECTABLE_CLASSES } from './storage-class';

const collator = new Intl.Collator('ja', { numeric: true, sensitivity: 'base' });

const CLASS_ORDER = new Map<StorageClass, number>(
  [...SELECTABLE_CLASSES, 'OTHER' as const].map((c, i) => [c, i]),
);

function compare(a: Entry, b: Entry, key: Sort['key']): number {
  switch (key) {
    case 'name':
      return collator.compare(a.name, b.name);
    case 'modified': {
      const x = a.lastModified ? Date.parse(a.lastModified) : 0;
      const y = b.lastModified ? Date.parse(b.lastModified) : 0;
      return x - y;
    }
    case 'size':
      return (a.type === 'file' ? a.size : 0) - (b.type === 'file' ? b.size : 0);
    case 'storageClass':
      return (
        (a.type === 'file' ? (CLASS_ORDER.get(a.storageClass) ?? 99) : -1) -
        (b.type === 'file' ? (CLASS_ORDER.get(b.storageClass) ?? 99) : -1)
      );
  }
}

export function sortEntries(entries: Entry[], sort: Sort): Entry[] {
  return [...entries].sort((a, b) => {
    const folder = (b.type === 'folder' ? 1 : 0) - (a.type === 'folder' ? 1 : 0);
    if (folder !== 0) return folder;
    const c = compare(a, b, sort.key) * sort.dir;
    // 同じ値のときは名前順で安定させる
    return c !== 0 ? c : collator.compare(a.name, b.name);
  });
}

// コンテキストメニュー MNU-01／02 の項目（03 §9.1〜9.2）。

import {
  ArchiveRestore,
  Copy,
  Download,
  FolderInput,
  FolderOpen,
  FolderPlus,
  FolderUp,
  History,
  Layers,
  Pencil,
  RefreshCw,
  RotateCcw,
  Trash2,
  Upload,
} from 'lucide-react';
import type { MenuEntry } from '@/components/ui/menu';
import { isArchived } from '@/features/context';
import { ja } from '@/lib/i18n/ja';
import type { Entry } from '@/lib/ipc';

const m = ja.menu;

/** MNU-01（項目のコンテキストメニュー）。 */
export function itemMenu(items: Entry[]): MenuEntry[] {
  const [first] = items;
  if (!first) return [];
  if (items.every((e) => e.deleted)) {
    return [
      { id: 'undelete', label: m.undelete, icon: RotateCcw },
      { separator: true, id: 's1' },
      { id: 'purge', label: m.purge, icon: Trash2, destructive: true },
    ];
  }
  const live = items.filter((e) => !e.deleted);
  const one = live.length === 1;
  const entries: MenuEntry[] = [];
  if (one && first.type === 'folder')
    entries.push({ id: 'open', label: m.open, icon: FolderOpen, shortcut: '⌘↓' });
  entries.push(
    { id: 'download', label: m.download, icon: Download, shortcut: '⌘D' },
    { id: 'move', label: m.move, icon: FolderInput },
  );
  if (one) entries.push({ id: 'rename', label: m.rename, icon: Pencil, shortcut: 'Return' });
  entries.push({ id: 'storageClass', label: m.storageClass, icon: Layers });
  if (live.some(isArchived)) entries.push({ id: 'restore', label: m.restore, icon: ArchiveRestore });
  if (one && first.type === 'file') entries.push({ id: 'versions', label: m.versions, icon: History });
  entries.push(
    { id: 'copyKey', label: m.copyKey, icon: Copy, shortcut: '⌥⌘C' },
    { separator: true, id: 's1' },
    {
      id: 'delete',
      label: live.length > 1 ? m.deleteMany(live.length) : m.delete,
      icon: Trash2,
      shortcut: '⌘⌫',
      destructive: true,
    },
  );
  return entries;
}

/** MNU-02（一覧の空白部分）。 */
export function blankMenu(): MenuEntry[] {
  return [
    { id: 'newFolder', label: m.newFolder, icon: FolderPlus, shortcut: '⇧⌘N' },
    { id: 'upload', label: m.upload, icon: Upload, shortcut: '⌘U' },
    { id: 'uploadFolder', label: m.uploadFolder, icon: FolderUp, shortcut: '⌥⌘U' },
    { separator: true, id: 's1' },
    { id: 'reload', label: m.reload, icon: RefreshCw, shortcut: '⌘R' },
  ];
}

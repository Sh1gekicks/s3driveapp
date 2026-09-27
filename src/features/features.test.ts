import { describe, expect, it } from 'vitest';
import type { Entry } from '@/lib/ipc';
import { summarize } from './batch';
import { blankMenu, itemMenu } from './browser/menus';

const err = { code: 'ACCESS_DENIED' as const, message: 'x', retryable: false };

describe('一括操作の結果の要約', () => {
  it.each([
    [{ succeeded: 3, skipped: [], failed: [] }, 'success'],
    [{ succeeded: 0, skipped: [{ key: 'a', reason: 'r' }], failed: [] }, 'skipped'],
    [{ succeeded: 2, skipped: [], failed: [{ key: 'a', error: err }] }, 'partial'],
    [{ succeeded: 0, skipped: [], failed: [{ key: 'a', error: err }] }, 'failed'],
    [{ succeeded: 0, skipped: [{ key: 'b', reason: 'r' }], failed: [{ key: 'a', error: err }] }, 'partial'],
  ])('%j → %s', (result, kind) => {
    expect(summarize(result)).toBe(kind);
  });
});

const file = (key: string, extra: Partial<Entry> = {}): Entry =>
  ({
    type: 'file',
    key,
    name: key.split('/').pop() ?? key,
    size: 1,
    lastModified: '2026-09-27T00:00:00Z',
    etag: '"e"',
    storageClass: 'STANDARD',
    restore: { state: 'notArchived' },
    deleted: false,
    ...extra,
  }) as Entry;
const folder: Entry = { type: 'folder', key: 'dir/', name: 'dir', lastModified: null, deleted: false };
const ids = (entries: ReturnType<typeof itemMenu>) =>
  entries.filter((e) => !('separator' in e)).map((e) => e.id);

describe('コンテキストメニュー（03 §9.1〜9.2）', () => {
  it('ファイル 1 項目', () => {
    expect(ids(itemMenu([file('a.txt')]))).toEqual([
      'download',
      'move',
      'rename',
      'storageClass',
      'versions',
      'copyKey',
      'delete',
    ]);
  });

  it('フォルダ 1 項目には「開く」', () => {
    expect(ids(itemMenu([folder]))[0]).toBe('open');
    expect(ids(itemMenu([folder]))).not.toContain('versions');
  });

  it('複数選択では名前の変更・バージョン履歴がなく、削除は件数付き', () => {
    const menu = itemMenu([file('a'), file('b')]);
    expect(ids(menu)).not.toContain('rename');
    expect(ids(menu)).not.toContain('versions');
    expect(menu.at(-1)).toMatchObject({ id: 'delete', label: '2 項目を削除', destructive: true });
  });

  it('取り出していないアーカイブがあれば「取り出し…」', () => {
    const archived = file('g', { storageClass: 'GLACIER', restore: { state: 'archived' } });
    expect(ids(itemMenu([archived]))).toContain('restore');
  });

  it('削除済みの項目は復元と完全に削除だけ', () => {
    expect(ids(itemMenu([file('a', { deleted: true })]))).toEqual(['undelete', 'purge']);
  });

  it('空白部分', () => {
    expect(ids(blankMenu())).toEqual(['newFolder', 'upload', 'uploadFolder', 'reload']);
  });
});

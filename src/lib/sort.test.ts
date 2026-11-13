import { describe, expect, it } from 'vitest';
import { extensionOf, fileKind } from './file-kind';
import type { Entry, StorageClass } from './ipc';
import { createIncrementalSorter, sortEntries } from './sort';
import { classLabel, isArchiveClass } from './storage-class';

const file = (
  name: string,
  size: number,
  lastModified: string,
  storageClass: StorageClass = 'STANDARD',
): Entry => ({
  type: 'file',
  key: name,
  name,
  size,
  lastModified,
  etag: '"x"',
  storageClass,
  restore: { state: 'notArchived' },
  deleted: false,
});
const folder = (name: string): Entry => ({
  type: 'folder',
  key: `${name}/`,
  name,
  lastModified: null,
  deleted: false,
});

describe('createIncrementalSorter（ページが届くたびの並べ替え）', () => {
  const sorts = [
    { key: 'name', dir: 1 },
    { key: 'name', dir: -1 },
    { key: 'modified', dir: -1 },
    { key: 'size', dir: 1 },
    { key: 'storageClass', dir: 1 },
  ] as const;
  // 3 ページ分。同じ値（大文字と小文字だけが違う名前、同じサイズ・日時）を含める
  const pages = [
    [
      file('b.txt', 10, '2026-09-02T00:00:00Z', 'GLACIER'),
      folder('zeta'),
      file('A.txt', 5, '2026-09-01T00:00:00Z'),
    ],
    [file('a.txt', 5, '2026-09-01T00:00:00Z'), folder('alpha'), file('a10.txt', 30, '2026-09-05T00:00:00Z')],
    [file('a2.txt', 10, '2026-09-02T00:00:00Z', 'STANDARD_IA'), folder('Beta')],
  ];

  it('末尾に追加されたときの結果は、全件を並べ替えた結果と同じ', () => {
    for (const sort of sorts) {
      const sorter = createIncrementalSorter();
      let loaded: Entry[] = [];
      for (const page of pages) {
        loaded = [...loaded, ...page];
        expect(sorter(loaded, sort)).toEqual(sortEntries(loaded, sort));
      }
    }
  });

  it('入力が同じなら前回の結果をそのまま返す', () => {
    const sorter = createIncrementalSorter();
    const loaded = pages.flat();
    const first = sorter(loaded, { key: 'name', dir: 1 });
    expect(sorter([...loaded], { key: 'name', dir: 1 })).toBe(first);
  });

  it('並び順の変更・項目の入れ替わり・別の一覧では全件を並べ替える', () => {
    const sorter = createIncrementalSorter();
    const loaded = pages.flat();
    sorter(loaded, { key: 'name', dir: 1 });
    expect(sorter(loaded, { key: 'size', dir: -1 })).toEqual(sortEntries(loaded, { key: 'size', dir: -1 }));
    // 取り直しで項目が入れ替わった（削除された）
    const changed = loaded.filter((e) => e.name !== 'b.txt');
    expect(sorter(changed, { key: 'size', dir: -1 })).toEqual(sortEntries(changed, { key: 'size', dir: -1 }));
    const other = [file('z.txt', 1, '2026-01-01T00:00:00Z'), file('y.txt', 2, '2026-01-02T00:00:00Z')];
    expect(sorter(other, { key: 'name', dir: 1 }).map((e) => e.name)).toEqual(['y.txt', 'z.txt']);
  });
});

describe('sortEntries（03 §5.4）', () => {
  const entries = [
    file('b.txt', 10, '2026-09-02T00:00:00Z', 'GLACIER'),
    folder('zeta'),
    file('a10.txt', 30, '2026-09-01T00:00:00Z', 'STANDARD_IA'),
    file('a2.txt', 20, '2026-09-03T00:00:00Z'),
    folder('alpha'),
  ];

  it('フォルダを常に先頭にし、名前は自然順', () => {
    expect(sortEntries(entries, { key: 'name', dir: 1 }).map((e) => e.name)).toEqual([
      'alpha',
      'zeta',
      'a2.txt',
      'a10.txt',
      'b.txt',
    ]);
  });

  it('降順でもフォルダは先頭', () => {
    expect(sortEntries(entries, { key: 'name', dir: -1 }).map((e) => e.name)).toEqual([
      'zeta',
      'alpha',
      'b.txt',
      'a10.txt',
      'a2.txt',
    ]);
  });

  it('サイズ・更新日・クラス', () => {
    expect(
      sortEntries(entries, { key: 'size', dir: -1 })
        .map((e) => e.name)
        .slice(2),
    ).toEqual(['a10.txt', 'a2.txt', 'b.txt']);
    expect(
      sortEntries(entries, { key: 'modified', dir: 1 })
        .map((e) => e.name)
        .slice(2),
    ).toEqual(['a10.txt', 'b.txt', 'a2.txt']);
    expect(
      sortEntries(entries, { key: 'storageClass', dir: 1 })
        .map((e) => e.name)
        .slice(2),
    ).toEqual(['a2.txt', 'a10.txt', 'b.txt']);
  });

  it('元の配列を変更しない', () => {
    const copy = [...entries];
    sortEntries(entries, { key: 'name', dir: 1 });
    expect(entries).toEqual(copy);
  });
});

describe('ファイル種別（02 §8.2）', () => {
  it.each([
    ['photo.HEIC', 'image'],
    ['movie.mov', 'video'],
    ['report.pdf', 'pdf'],
    ['data.csv', 'sheet'],
    ['backup.tar.gz', 'archive'],
    ['main.rs', 'code'],
    ['notes', 'doc'],
    ['.env', 'doc'],
  ])('%s → %s', (name, kind) => {
    expect(fileKind(name)).toBe(kind);
  });
  it('フォルダ', () => {
    expect(fileKind('projects/')).toBe('folder');
    expect(fileKind('x', true)).toBe('folder');
  });
  it('拡張子', () => {
    expect(extensionOf('a.B.Txt')).toBe('txt');
    expect(extensionOf('.hidden')).toBe('');
    expect(extensionOf('trailing.')).toBe('');
  });
});

describe('ストレージクラス', () => {
  it('表示名', () => {
    expect(classLabel('GLACIER_IR')).toBe('Glacier Instant Retrieval');
    expect(classLabel('GLACIER_IR', true)).toBe('Glacier IR');
    expect(classLabel('OTHER')).toBe('その他');
  });
  it('取り出しが必要なクラス', () => {
    expect(isArchiveClass('GLACIER')).toBe(true);
    expect(isArchiveClass('DEEP_ARCHIVE')).toBe(true);
    expect(isArchiveClass('GLACIER_IR')).toBe(false);
  });
});

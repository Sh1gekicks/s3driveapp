import { describe, expect, it } from 'vitest';
import { extensionOf, fileKind } from './file-kind';
import type { Entry, StorageClass } from './ipc';
import { sortEntries } from './sort';
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

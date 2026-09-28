import { beforeEach, describe, expect, it } from 'vitest';
import type { SearchEntry, SearchResult } from '@/lib/ipc';
import { parentPrefix, useNavStore } from './nav';
import { filterCount, NO_FILTERS, nextSearchOffset, toSearchQuery, useUiStore } from './ui';

describe('useNavStore（戻る／進む）', () => {
  beforeEach(() => useNavStore.getState().openConnection('c1'));

  it('移動・戻る・進む', () => {
    const nav = useNavStore.getState();
    nav.navigate('projects/');
    nav.navigate('projects/2026/');
    expect(useNavStore.getState().back).toEqual(['', 'projects/']);
    useNavStore.getState().goBack();
    expect(useNavStore.getState().prefix).toBe('projects/');
    expect(useNavStore.getState().forward).toEqual(['projects/2026/']);
    useNavStore.getState().goForward();
    expect(useNavStore.getState().prefix).toBe('projects/2026/');
    expect(useNavStore.getState().forward).toEqual([]);
  });

  it('新しく移動すると進むの履歴は消える', () => {
    const nav = useNavStore.getState();
    nav.navigate('a/');
    nav.navigate('a/b/');
    useNavStore.getState().goBack();
    useNavStore.getState().navigate('c/');
    expect(useNavStore.getState().forward).toEqual([]);
  });

  it('親フォルダ', () => {
    expect(parentPrefix('a/b/c/')).toBe('a/b/');
    expect(parentPrefix('a/b.txt')).toBe('a/');
    expect(parentPrefix('a/')).toBe('');
    useNavStore.getState().navigate('a/b/');
    useNavStore.getState().goUp();
    expect(useNavStore.getState().prefix).toBe('a/');
  });

  it('接続を開くと履歴を消す', () => {
    useNavStore.getState().navigate('x/');
    useNavStore.getState().openConnection('c2', 'y/');
    expect(useNavStore.getState()).toMatchObject({ connectionId: 'c2', prefix: 'y/', back: [], forward: [] });
  });
});

describe('useUiStore（選択）', () => {
  const order = ['a', 'b', 'c', 'd', 'e'];
  beforeEach(() => useUiStore.getState().setSelection([]));

  it('クリック・⌘クリック・⇧クリック', () => {
    const ui = useUiStore.getState();
    ui.select('b', {}, order);
    expect(useUiStore.getState().selection).toEqual({ keys: ['b'], anchor: 'b' });
    useUiStore.getState().select('d', { meta: true }, order);
    expect(useUiStore.getState().selection.keys).toEqual(['b', 'd']);
    useUiStore.getState().select('b', { meta: true }, order);
    expect(useUiStore.getState().selection.keys).toEqual(['d']);
    useUiStore.getState().select('a', { shift: true }, order);
    // 起点は最後に ⌘クリックした項目
    expect(useUiStore.getState().selection.keys).toEqual(['a', 'b']);
    useUiStore.getState().select(null, {}, order);
    expect(useUiStore.getState().selection.keys).toEqual([]);
  });

  it('↑↓ と ⇧↑↓', () => {
    const ui = useUiStore.getState();
    ui.moveSelection(1, false, order);
    expect(useUiStore.getState().selection.keys).toEqual(['a']);
    useUiStore.getState().moveSelection(1, true, order);
    useUiStore.getState().moveSelection(1, true, order);
    expect(useUiStore.getState().selection.keys).toEqual(['a', 'b', 'c']);
    useUiStore.getState().moveSelection(-1, true, order);
    expect(useUiStore.getState().selection.keys).toEqual(['a', 'b']);
    useUiStore.getState().moveSelection(-1, false, order);
    expect(useUiStore.getState().selection.keys).toEqual(['a']);
    useUiStore.getState().moveSelection(-1, false, order);
    expect(useUiStore.getState().selection.keys).toEqual(['a']);
  });

  it('すべてを選択', () => {
    useUiStore.getState().selectAll(order);
    expect(useUiStore.getState().selection.keys).toEqual(order);
  });

  it('並べ替えの切り替え', () => {
    useUiStore.getState().setSort({ key: 'name', dir: 1 });
    useUiStore.getState().toggleSort('name');
    expect(useUiStore.getState().sort).toEqual({ key: 'name', dir: -1 });
    useUiStore.getState().toggleSort('size');
    expect(useUiStore.getState().sort).toEqual({ key: 'size', dir: 1 });
  });
});

describe('検索条件（04 §10.2）', () => {
  it('フィルタの数', () => {
    expect(filterCount(NO_FILTERS)).toBe(0);
    expect(filterCount({ ...NO_FILTERS, ext: '  ' })).toBe(0);
    expect(filterCount({ ...NO_FILTERS, kind: 'pdf', ext: 'pdf' })).toBe(2);
  });

  it('指定されたものだけを渡す', () => {
    expect(toSearchQuery(' report ', NO_FILTERS, { key: 'name', dir: 1 })).toEqual({
      text: 'report',
      sort: { key: 'name', dir: 1 },
      offset: 0,
      limit: 1000,
    });
    expect(
      toSearchQuery(
        '',
        { kind: 'pdf', ext: '.PDF', size: 'lt1', date: '7d', storageClass: 'GLACIER' },
        { key: 'size', dir: -1 },
      ),
    ).toMatchObject({ kind: 'pdf', ext: 'pdf', size: 'lt1', date: '7d', storageClass: 'GLACIER' });
  });

  describe('結果の続きの取得（1,000 件ずつ）', () => {
    const file = (key: string): SearchEntry => ({
      entry: {
        type: 'file',
        key,
        name: key,
        size: 1,
        lastModified: '2026-09-27T00:00:00Z',
        etag: 'e',
        storageClass: 'STANDARD',
        restore: { state: 'notArchived' },
        deleted: false,
      },
      parent: '',
    });
    const folder = (key: string): SearchEntry => ({
      entry: { type: 'folder', key, name: key, lastModified: null, deleted: false },
      parent: '',
    });
    const page = (entries: SearchEntry[], total: number): SearchResult => ({
      entries,
      total,
      index: { state: 'ready', objectCount: 0, lastScanAt: null, sizeBytes: 0, progress: null },
    });

    it('フォルダは最初のページにだけ含まれるため、読み込んだファイルの件数を次の offset にする', () => {
      // フォルダ 2 件 + ファイル 3 件のうち、最初のページでファイル 2 件
      const first = page([folder('a/'), folder('b/'), file('1'), file('2')], 5);
      expect(nextSearchOffset([first])).toBe(2);
      expect(nextSearchOffset([first, page([file('3')], 5)])).toBeUndefined();
    });

    it('すべて読み込んだ・空のページが返ったら終わり', () => {
      expect(nextSearchOffset([page([file('1')], 1)])).toBeUndefined();
      expect(nextSearchOffset([page([file('1')], 3), page([], 3)])).toBeUndefined();
    });
  });
});

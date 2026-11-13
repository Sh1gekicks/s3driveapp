// 一覧の並べ替え（03 §5.4）。フォルダを常に先頭にし、その中で選択した列の順に並べる。

import type { Entry, Sort, StorageClass } from './ipc';
import { SELECTABLE_CLASSES } from './storage-class';

const collator = new Intl.Collator('ja', { numeric: true, sensitivity: 'base' });

const CLASS_ORDER = new Map<StorageClass, number>(
  [...SELECTABLE_CLASSES, 'OTHER' as const].map((c, i) => [c, i]),
);

/** 更新日のタイムスタンプ。比較のたびに日時を解析しないよう、項目ごとに 1 回だけ求める。 */
const timestamps = new WeakMap<Entry, number>();

function timestamp(e: Entry): number {
  let t = timestamps.get(e);
  if (t === undefined) {
    t = e.lastModified ? Date.parse(e.lastModified) : 0;
    timestamps.set(e, t);
  }
  return t;
}

function compare(a: Entry, b: Entry, key: Sort['key']): number {
  switch (key) {
    case 'name':
      return collator.compare(a.name, b.name);
    case 'modified':
      return timestamp(a) - timestamp(b);
    case 'size':
      return (a.type === 'file' ? a.size : 0) - (b.type === 'file' ? b.size : 0);
    case 'storageClass':
      return (
        (a.type === 'file' ? (CLASS_ORDER.get(a.storageClass) ?? 99) : -1) -
        (b.type === 'file' ? (CLASS_ORDER.get(b.storageClass) ?? 99) : -1)
      );
  }
}

function comparator(sort: Sort): (a: Entry, b: Entry) => number {
  return (a, b) => {
    const folder = (b.type === 'folder' ? 1 : 0) - (a.type === 'folder' ? 1 : 0);
    if (folder !== 0) return folder;
    const c = compare(a, b, sort.key) * sort.dir;
    // 同じ値のときは名前順で安定させる
    return c !== 0 ? c : collator.compare(a.name, b.name);
  };
}

export function sortEntries(entries: Entry[], sort: Sort): Entry[] {
  return [...entries].sort(comparator(sort));
}

/** 並べ替え済みの 2 つの配列を併合する。同じ値は `head` の項目を先にする（安定な並べ替えと同じ結果になる）。 */
function merge(head: Entry[], tail: Entry[], cmp: (a: Entry, b: Entry) => number): Entry[] {
  const out: Entry[] = [];
  let i = 0;
  let j = 0;
  while (i < head.length && j < tail.length) {
    const a = head[i] as Entry;
    const b = tail[j] as Entry;
    if (cmp(a, b) <= 0) {
      out.push(a);
      i++;
    } else {
      out.push(b);
      j++;
    }
  }
  while (i < head.length) out.push(head[i++] as Entry);
  while (j < tail.length) out.push(tail[j++] as Entry);
  return out;
}

/**
 * 前回の結果を使う並べ替え。結果は `sortEntries` と同じ。
 *
 * 一覧は 1,000 件ずつ届いて末尾に追加される（04 §3.1）。そのたびに全件を並べ替えると、10 万件のフォルダでは
 * 100 回の全件の並べ替えになる。前回の入力に項目が追加されただけなら、追加分だけを並べ替えて前回の結果と
 * 併合する。それ以外（別のフォルダ、並び順の変更、項目の入れ替わり）は全件を並べ替える。
 */
export function createIncrementalSorter(): (entries: Entry[], sort: Sort) => Entry[] {
  let prev: { input: Entry[]; sort: Sort; output: Entry[] } | null = null;
  return (entries, sort) => {
    let output: Entry[];
    const appended =
      prev !== null &&
      prev.sort.key === sort.key &&
      prev.sort.dir === sort.dir &&
      entries.length >= prev.input.length &&
      prev.input.every((e, i) => e === entries[i]);
    if (prev !== null && appended) {
      output =
        entries.length === prev.input.length
          ? prev.output
          : merge(prev.output, sortEntries(entries.slice(prev.input.length), sort), comparator(sort));
    } else {
      output = sortEntries(entries, sort);
    }
    prev = { input: entries, sort, output };
    return output;
  };
}

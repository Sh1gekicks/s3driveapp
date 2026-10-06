// リスト表示の列（03 §5.4）。名前の列は残りの幅を使い、ほかの列は保存した幅（見出しの境界のドラッグで変える）を使う。
// 一覧が狭く名前の列が最小の幅を下回るときは、表示する幅だけを重要度の低い列から縮めて一覧に収める。

import type { ColumnWidths, SortKey } from '@/lib/ipc';
import { COLUMN_WIDTH } from '@/stores/ui';

export type ColumnKey = keyof ColumnWidths;

/** 名前の列の最小の幅（px）。幅が狭いとき（01 §5.5）は小さくする。 */
export const NAME_MIN_WIDTH = { wide: 200, narrow: 140 } as const;

/** 列で並べ替えるときのキー（種類では並べ替えない）。 */
export const COLUMN_SORT_KEY: Record<ColumnKey, SortKey | undefined> = {
  modified: 'modified',
  size: 'size',
  kind: undefined,
  storageClass: 'storageClass',
};

/** 名前の列の右に並べる列。幅が狭いときは「種類」を省く（01 §5.5）。 */
export function visibleColumns(narrow: boolean): ColumnKey[] {
  return narrow ? ['modified', 'size', 'storageClass'] : ['modified', 'size', 'kind', 'storageClass'];
}

/** 一覧に収まらないときに縮める順（種類 → ストレージクラス → サイズ → 更新日）。 */
const SHRINK_ORDER: ColumnKey[] = ['kind', 'storageClass', 'size', 'modified'];

/**
 * 表示する列の幅。`columns` の幅の合計が `room`（名前の列の最小の幅を除いた、列に使える幅）を超えるときは、
 * SHRINK_ORDER の順に最小の幅まで縮める。すべて最小の幅でも収まらなければそのまま（一覧を横にスクロールする）。
 */
export function fitColumns(widths: ColumnWidths, columns: ColumnKey[], room: number): ColumnWidths {
  const fitted = { ...widths };
  let over = columns.reduce((sum, k) => sum + widths[k], 0) - room;
  for (const k of SHRINK_ORDER) {
    if (over <= 0) break;
    if (!columns.includes(k)) continue;
    const cut = Math.min(over, fitted[k] - COLUMN_WIDTH.min);
    if (cut <= 0) continue;
    fitted[k] -= cut;
    over -= cut;
  }
  return fitted;
}

/** 列 `column` を広げられる上限（名前の列が最小の幅になるまで）。 */
export function maxColumnWidth(fitted: ColumnWidths, columns: ColumnKey[], room: number, column: ColumnKey) {
  const free = room - columns.reduce((sum, k) => sum + fitted[k], 0);
  return Math.max(
    COLUMN_WIDTH.min,
    Math.min(COLUMN_WIDTH.max, fitted[column] + Math.max(0, Math.floor(free))),
  );
}

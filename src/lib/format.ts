// 書式（02 §9.2）。サイズは 10 進接頭辞、日時は YYYY/MM/DD HH:mm、金額は米ドル。

const UNITS = ['KB', 'MB', 'GB', 'TB', 'PB'] as const;

/** サイズ（1 KB = 1,000 B。100 以上は整数、未満は小数 1 桁）。 */
export function formatSize(bytes: number | null | undefined): string {
  if (bytes == null || Number.isNaN(bytes)) return '—';
  if (bytes < 1000) return `${bytes} B`;
  let value = bytes;
  let unit = -1;
  do {
    value /= 1000;
    unit++;
  } while (value >= 1000 && unit < UNITS.length - 1);
  // 丸めた結果が 1000 になる場合は次の単位にする（999.96 KB → 1.0 MB）
  if (Math.round(value) >= 1000 && unit < UNITS.length - 1) {
    value /= 1000;
    unit++;
  }
  const text = value >= 100 ? String(Math.round(value)) : value.toFixed(1);
  return `${text} ${UNITS[unit]}`;
}

const numberFormat = new Intl.NumberFormat('ja-JP');

/** 桁区切りの数値。 */
export function formatNumber(n: number): string {
  return numberFormat.format(n);
}

/** サイズ（詳細）: 「4.2 MB（4,200,000 バイト）」。 */
export function formatSizeDetail(bytes: number): string {
  return `${formatSize(bytes)}（${formatNumber(bytes)} バイト）`;
}

const pad = (n: number) => String(n).padStart(2, '0');

/** 日時（ローカル時刻の YYYY/MM/DD HH:mm）。 */
export function formatDate(value: string | Date | null | undefined): string {
  if (value == null || value === '') return '—';
  const d = typeof value === 'string' ? new Date(value) : value;
  if (Number.isNaN(d.getTime())) return '—';
  return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

const tinyUsd = new Intl.NumberFormat('en-US', { maximumSignificantDigits: 2 });

/**
 * 金額（米ドル。1 ドル以上は小数 2 桁、1 ドル未満は小数 3 桁）。
 * 0.001 ドル未満は「$0.000」と区別できるよう有効数字 2 桁にする（$0.00029）。
 */
export function formatUsd(amount: number | null | undefined): string {
  if (amount == null || Number.isNaN(amount)) return '—';
  const abs = Math.abs(amount);
  const sign = amount < 0 ? '-' : '';
  if (abs > 0 && abs < 0.001) return `${sign}$${tinyUsd.format(abs)}`;
  return `${sign}$${abs.toFixed(abs < 1 ? 3 : 2)}`;
}

/** 割合（小数 1 桁）。 */
export function formatPercent(ratio: number): string {
  return `${(ratio * 100).toFixed(1)}%`;
}

/** 増減の割合（「+3.2%」「-1.0%」）。 */
export function formatDelta(ratio: number): string {
  const value = (ratio * 100).toFixed(1);
  return ratio > 0 ? `+${value}%` : `${value}%`;
}

/** 件数（「12 項目」）。 */
export function formatCount(n: number, unit: '項目' | '件' | 'オブジェクト' = '項目'): string {
  return `${formatNumber(n)} ${unit}`;
}

/** 経過時間（「3 分前」など。インデックスの鮮度表示）。 */
export function formatRelative(value: string | null | undefined, now: Date = new Date()): string {
  if (!value) return '—';
  const minutes = Math.floor((now.getTime() - new Date(value).getTime()) / 60000);
  if (minutes < 1) return 'たった今';
  if (minutes < 60) return `${minutes} 分前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 時間前`;
  return `${Math.floor(hours / 24)} 日前`;
}

/** 転送速度（「12.4 MB/秒」）。 */
export function formatSpeed(bytesPerSec: number): string {
  return `${formatSize(Math.round(bytesPerSec))}/秒`;
}

/** 残り時間（「約 3 分」）。 */
export function formatEta(seconds: number | null | undefined): string {
  if (seconds == null) return '';
  if (seconds < 60) return `残り約 ${Math.max(1, Math.round(seconds))} 秒`;
  if (seconds < 3600) return `残り約 ${Math.round(seconds / 60)} 分`;
  return `残り約 ${Math.round(seconds / 3600)} 時間`;
}

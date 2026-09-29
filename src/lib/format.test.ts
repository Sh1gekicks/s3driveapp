import { describe, expect, it } from 'vitest';
import {
  formatDate,
  formatDelta,
  formatEta,
  formatPercent,
  formatRelative,
  formatSize,
  formatSizeDetail,
  formatUsd,
} from './format';

describe('formatSize（02 §9.2: 10 進接頭辞）', () => {
  it.each([
    [null, '—'],
    [0, '0 B'],
    [999, '999 B'],
    [1000, '1.0 KB'],
    [2100, '2.1 KB'],
    [184_000, '184 KB'],
    [4_200_000, '4.2 MB'],
    [999_960, '1.0 MB'],
    [2_300_000_000, '2.3 GB'],
    [1.5e15, '1.5 PB'],
  ])('%s → %s', (bytes, expected) => {
    expect(formatSize(bytes)).toBe(expected);
  });

  it('詳細表示はバイト数を併記する', () => {
    expect(formatSizeDetail(4_200_000)).toBe('4.2 MB（4,200,000 バイト）');
  });
});

describe('formatDate', () => {
  it('ローカル時刻の YYYY/MM/DD HH:mm', () => {
    expect(formatDate(new Date(2026, 8, 7, 4, 5))).toBe('2026/09/07 04:05');
  });
  it('空・不正な値は —', () => {
    expect(formatDate(null)).toBe('—');
    expect(formatDate('')).toBe('—');
    expect(formatDate('not a date')).toBe('—');
  });
});

describe('金額・割合', () => {
  it('1 ドル未満は小数 3 桁', () => {
    expect(formatUsd(0.257)).toBe('$0.257');
    expect(formatUsd(6.6612)).toBe('$6.66');
    expect(formatUsd(-1.5)).toBe('-$1.50');
    expect(formatUsd(null)).toBe('—');
  });
  it('0.001 ドル未満は有効数字 2 桁（$0.000 と区別する）', () => {
    expect(formatUsd(0)).toBe('$0.000');
    expect(formatUsd(0.00028738)).toBe('$0.00029');
    expect(formatUsd(0.00099)).toBe('$0.00099');
    expect(formatUsd(0.0000036)).toBe('$0.0000036');
    expect(formatUsd(-0.0002)).toBe('-$0.0002');
  });
  it('割合と増減', () => {
    expect(formatPercent(0.1234)).toBe('12.3%');
    expect(formatDelta(0.032)).toBe('+3.2%');
    expect(formatDelta(-0.01)).toBe('-1.0%');
  });
});

describe('時間', () => {
  const now = new Date('2026-09-27T12:00:00Z');
  it('経過時間', () => {
    expect(formatRelative('2026-09-27T11:59:40Z', now)).toBe('たった今');
    expect(formatRelative('2026-09-27T11:57:00Z', now)).toBe('3 分前');
    expect(formatRelative('2026-09-27T09:00:00Z', now)).toBe('3 時間前');
    expect(formatRelative('2026-09-25T12:00:00Z', now)).toBe('2 日前');
  });
  it('残り時間', () => {
    expect(formatEta(null)).toBe('');
    expect(formatEta(0.2)).toBe('残り約 1 秒');
    expect(formatEta(150)).toBe('残り約 3 分');
    expect(formatEta(7200)).toBe('残り約 2 時間');
  });
});

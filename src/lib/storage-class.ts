// ストレージクラスの表示名と説明（DS: StorageClassBadge.jsx の STORAGE_CLASSES、data.js の CLASS_INFO。03 DLG-04）。

import type { StorageClass } from './ipc/bindings/StorageClass';

export type SelectableClass = Exclude<StorageClass, 'OTHER'>;

export interface StorageClassInfo {
  label: string;
  short: string;
  /** --sc-* のトークン名 */
  token: string;
  description: string;
  /** 最低保存期間（日） */
  minDays: number | null;
  /** 取り出しにかかる時間 */
  retrieval: string;
  /** 取り出しに時間がかかる（アーカイブ） */
  slow: boolean;
  /** 128 KB 未満でも 128 KB として課金される */
  minSize128k: boolean;
}

export const STORAGE_CLASSES: Record<SelectableClass, StorageClassInfo> = {
  STANDARD: {
    label: 'Standard',
    short: 'Standard',
    token: 'standard',
    description: '頻繁にアクセスするデータ向け',
    minDays: null,
    retrieval: 'ミリ秒',
    slow: false,
    minSize128k: false,
  },
  INTELLIGENT_TIERING: {
    label: 'Intelligent-Tiering',
    short: 'Int. Tiering',
    token: 'intelligent-tiering',
    description: 'アクセス頻度に応じて自動で階層を移動',
    minDays: null,
    retrieval: 'ミリ秒',
    slow: false,
    minSize128k: false,
  },
  STANDARD_IA: {
    label: 'Standard-IA',
    short: 'Standard-IA',
    token: 'standard-ia',
    description: 'アクセス頻度は低いが即時取得が必要なデータ',
    minDays: 30,
    retrieval: 'ミリ秒',
    slow: false,
    minSize128k: true,
  },
  ONEZONE_IA: {
    label: 'One Zone-IA',
    short: 'One Zone-IA',
    token: 'onezone-ia',
    description: '単一 AZ に保存。再作成可能なデータ向け',
    minDays: 30,
    retrieval: 'ミリ秒',
    slow: false,
    minSize128k: true,
  },
  GLACIER_IR: {
    label: 'Glacier Instant Retrieval',
    short: 'Glacier IR',
    token: 'glacier-ir',
    description: '四半期に一度程度のアクセス。即時取得',
    minDays: 90,
    retrieval: 'ミリ秒',
    slow: false,
    minSize128k: true,
  },
  GLACIER: {
    label: 'Glacier Flexible Retrieval',
    short: 'Glacier',
    token: 'glacier',
    description: '年に数回のアクセス。取り出しに数分〜数時間',
    minDays: 90,
    retrieval: '数分〜12 時間',
    slow: true,
    minSize128k: false,
  },
  DEEP_ARCHIVE: {
    label: 'Glacier Deep Archive',
    short: 'Deep Archive',
    token: 'deep-archive',
    description: '長期保管用。最も低コスト',
    minDays: 180,
    retrieval: '12〜48 時間',
    slow: true,
    minSize128k: false,
  },
};

export const SELECTABLE_CLASSES = Object.keys(STORAGE_CLASSES) as SelectableClass[];

export function classInfo(value: StorageClass): StorageClassInfo | null {
  return value === 'OTHER' ? null : STORAGE_CLASSES[value];
}

export function classLabel(value: StorageClass, short = false): string {
  const info = classInfo(value);
  if (!info) return 'その他';
  return short ? info.short : info.label;
}

/** 識別色（CSS 変数）。「その他」は灰色。 */
export function classColor(value: StorageClass): string {
  const info = classInfo(value);
  return info ? `var(--sc-${info.token})` : 'var(--gray-400)';
}

/** 取り出し（RestoreObject）が必要になりうるクラス。 */
export function isArchiveClass(value: StorageClass): boolean {
  return value === 'GLACIER' || value === 'DEEP_ARCHIVE';
}

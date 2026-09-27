// ファイル種別の判定（02 §8.2。DS: components/data/FileIcon.jsx と同じ対応）。

import type { FileKind } from './ipc/bindings/FileKind';

export type Kind = FileKind | 'folder';

const EXT: Record<string, FileKind> = {
  jpg: 'image',
  jpeg: 'image',
  png: 'image',
  gif: 'image',
  heic: 'image',
  webp: 'image',
  svg: 'image',
  mp4: 'video',
  mov: 'video',
  mp3: 'audio',
  wav: 'audio',
  m4a: 'audio',
  pdf: 'pdf',
  csv: 'sheet',
  xlsx: 'sheet',
  numbers: 'sheet',
  zip: 'archive',
  gz: 'archive',
  tar: 'archive',
  js: 'code',
  ts: 'code',
  json: 'code',
  py: 'code',
  rs: 'code',
  html: 'code',
};

/** 拡張子（小文字、先頭の . を除く）。拡張子がなければ空文字。 */
export function extensionOf(name: string): string {
  const dot = name.lastIndexOf('.');
  if (dot <= 0 || dot === name.length - 1) return '';
  return name.slice(dot + 1).toLowerCase();
}

export function fileKind(name: string, isFolder = false): Kind {
  if (isFolder || name.endsWith('/')) return 'folder';
  return EXT[extensionOf(name)] ?? 'doc';
}

/** 種類の表示名。 */
export const KIND_LABEL: Record<Kind, string> = {
  folder: 'フォルダ',
  image: '画像',
  video: 'ムービー',
  audio: 'オーディオ',
  pdf: 'PDF 書類',
  sheet: 'スプレッドシート',
  archive: 'アーカイブ',
  code: 'ソースコード',
  doc: '書類',
};

/** フィルタの選択肢（フォルダを除く）。 */
export const FILE_KINDS: FileKind[] = ['image', 'video', 'audio', 'pdf', 'sheet', 'archive', 'code', 'doc'];

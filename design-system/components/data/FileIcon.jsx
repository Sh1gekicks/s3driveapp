import React from 'react';
import { Icon } from '../core/Icon.jsx';
const MAP = {
  folder: ['folder', 'folder'], image: ['image', 'image'], video: ['film', 'video'], audio: ['music', 'audio'],
  pdf: ['file-text', 'pdf'], sheet: ['file-spreadsheet', 'sheet'], archive: ['file-archive', 'archive'], code: ['file-code', 'code'], doc: ['file', 'doc'],
};
const EXT = { jpg: 'image', jpeg: 'image', png: 'image', gif: 'image', heic: 'image', webp: 'image', svg: 'image', mp4: 'video', mov: 'video', mp3: 'audio', wav: 'audio', m4a: 'audio', pdf: 'pdf', csv: 'sheet', xlsx: 'sheet', numbers: 'sheet', zip: 'archive', gz: 'archive', tar: 'archive', js: 'code', ts: 'code', json: 'code', py: 'code', rs: 'code', html: 'code' };
export function fileKind(name = '', isFolder) {
  if (isFolder || name.endsWith('/')) return 'folder';
  const ext = name.split('.').pop().toLowerCase();
  return EXT[ext] || 'doc';
}
export function FileIcon({ name, kind, folder, size = 16, style }) {
  const k = kind || fileKind(name, folder);
  const [icon, tok] = MAP[k] || MAP.doc;
  const c = 'var(--ft-' + tok + ')';
  return <Icon name={icon} size={size} color={c} fill={'color-mix(in oklch, ' + c + ' 22%, transparent)'} style={style} />;
}

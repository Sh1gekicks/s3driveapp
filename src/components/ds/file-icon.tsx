import {
  File,
  FileArchive,
  FileCode,
  FileSpreadsheet,
  FileText,
  Film,
  Folder,
  Image,
  type LucideIcon,
  Music,
} from 'lucide-react';
import { fileKind, type Kind } from '@/lib/file-kind';
import { Icon } from './icon';

// DS: components/data/FileIcon.jsx。拡張子から種別を判定し、Lucide のグリフ＋ --ft-* 色＋ 22% の塗り（02 §8.2）。

const MAP: Record<Kind, [LucideIcon, string]> = {
  folder: [Folder, 'folder'],
  image: [Image, 'image'],
  video: [Film, 'video'],
  audio: [Music, 'audio'],
  pdf: [FileText, 'pdf'],
  sheet: [FileSpreadsheet, 'sheet'],
  archive: [FileArchive, 'archive'],
  code: [FileCode, 'code'],
  doc: [File, 'doc'],
};

export interface FileIconProps {
  name?: string;
  folder?: boolean;
  kind?: Kind;
  size?: number;
  className?: string;
}

export function FileIcon({ name = '', folder, kind, size = 16, className }: FileIconProps) {
  const [glyph, token] = MAP[kind ?? fileKind(name, folder)];
  const color = `var(--ft-${token})`;
  return (
    <Icon
      icon={glyph}
      size={size}
      color={color}
      fill={`color-mix(in oklch, ${color} 22%, transparent)`}
      className={className}
      style={{ flexShrink: 0 }}
    />
  );
}

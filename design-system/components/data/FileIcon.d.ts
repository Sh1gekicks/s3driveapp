export type FileKind = 'folder' | 'image' | 'video' | 'audio' | 'pdf' | 'sheet' | 'archive' | 'code' | 'doc';
export interface FileIconProps {
  /** File name / key — kind is inferred from the extension */
  name?: string;
  kind?: FileKind;
  folder?: boolean;
  size?: number;
  style?: React.CSSProperties;
}
export declare function fileKind(name?: string, isFolder?: boolean): FileKind;
export declare function FileIcon(props: FileIconProps): JSX.Element;

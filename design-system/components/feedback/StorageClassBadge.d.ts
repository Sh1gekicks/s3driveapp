export type StorageClass = 'STANDARD' | 'INTELLIGENT_TIERING' | 'STANDARD_IA' | 'ONEZONE_IA' | 'GLACIER_IR' | 'GLACIER' | 'DEEP_ARCHIVE';
export interface StorageClassBadgeProps {
  /** AWS StorageClass enum value */
  value?: StorageClass;
  /** Short label (default true) */
  short?: boolean;
  /** No pill background — dot + text only (table cells) */
  plain?: boolean;
  style?: React.CSSProperties;
}
export declare const STORAGE_CLASSES: Record<StorageClass, { label: string; short: string; token: string }>;
export declare function StorageClassBadge(props: StorageClassBadgeProps): JSX.Element;

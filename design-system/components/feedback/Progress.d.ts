export interface ProgressSegment { value: number; color: string; label?: string; }
export interface ProgressProps {
  value?: number;
  max?: number;
  /** Stacked segments (e.g. usage by storage class) */
  segments?: ProgressSegment[];
  /** px, default 6 */
  height?: number;
  color?: string;
  style?: React.CSSProperties;
}
export declare function Progress(props: ProgressProps): JSX.Element;

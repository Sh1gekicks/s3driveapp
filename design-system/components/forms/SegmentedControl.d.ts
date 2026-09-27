export interface SegmentOption { value: string; label: string; icon?: string; iconOnly?: boolean; }
export interface SegmentedControlProps {
  options: SegmentOption[];
  value: string;
  onChange?: (value: string) => void;
  /** Stretch to container width; segments share space equally */
  block?: boolean;
  style?: React.CSSProperties;
}
export declare function SegmentedControl(props: SegmentedControlProps): JSX.Element;

export interface SwitchProps {
  checked?: boolean;
  onChange?: (checked: boolean) => void;
  /** When set, renders label left / switch right (settings row) */
  label?: React.ReactNode;
  disabled?: boolean;
  style?: React.CSSProperties;
}
export declare function Switch(props: SwitchProps): JSX.Element;

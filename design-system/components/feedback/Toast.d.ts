export interface ToastProps {
  icon?: string;
  tone?: 'default' | 'success' | 'destructive' | 'warning';
  title: React.ReactNode;
  description?: React.ReactNode;
  /** 0–100 → shows a thin progress bar (transfers) */
  progress?: number;
  action?: React.ReactNode;
  onClose?: () => void;
  style?: React.CSSProperties;
}
export declare function Toast(props: ToastProps): JSX.Element;

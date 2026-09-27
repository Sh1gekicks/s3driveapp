/**
 * @startingPoint section="Feedback" subtitle="Modal dialog — confirm, form, destructive" viewport="700x340"
 */
export interface DialogProps {
  open?: boolean;
  /** Lucide icon in a tinted tile */
  icon?: string;
  tone?: 'default' | 'destructive';
  title: React.ReactNode;
  description?: React.ReactNode;
  children?: React.ReactNode;
  /** Right-aligned buttons; primary last */
  footer?: React.ReactNode;
  onClose?: () => void;
  /** px, default 420 */
  width?: number;
}
export declare function Dialog(props: DialogProps): JSX.Element | null;

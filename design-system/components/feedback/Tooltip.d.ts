export interface TooltipProps {
  content: React.ReactNode;
  side?: 'top' | 'bottom';
  /** Force open (static docs) */
  open?: boolean;
  children: React.ReactNode;
}
export declare function Tooltip(props: TooltipProps): JSX.Element;

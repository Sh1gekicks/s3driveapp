/**
 * @startingPoint section="Actions" subtitle="Push button — default, secondary, outline, ghost, destructive, link" viewport="700x220"
 */
export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'default' | 'secondary' | 'outline' | 'ghost' | 'destructive' | 'link';
  /** sm 24px · md 28px (default) · lg 32px */
  size?: 'sm' | 'md' | 'lg';
  /** Lucide icon name before the label */
  icon?: string;
  iconRight?: string;
}
export declare function Button(props: ButtonProps): JSX.Element;

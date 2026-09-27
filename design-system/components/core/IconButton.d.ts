export interface IconButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** Lucide icon name */
  icon: string;
  /** Accessible label + native tooltip (required) */
  label: string;
  variant?: 'ghost' | 'outline' | 'secondary' | 'default' | 'destructive';
  size?: 'sm' | 'md' | 'lg';
  /** Toggled state (e.g. current view mode) */
  active?: boolean;
}
export declare function IconButton(props: IconButtonProps): JSX.Element;

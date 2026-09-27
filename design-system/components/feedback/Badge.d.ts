export interface BadgeProps {
  variant?: 'default' | 'secondary' | 'outline' | 'success' | 'warning' | 'destructive';
  icon?: string;
  children?: React.ReactNode;
  style?: React.CSSProperties;
}
export declare function Badge(props: BadgeProps): JSX.Element;

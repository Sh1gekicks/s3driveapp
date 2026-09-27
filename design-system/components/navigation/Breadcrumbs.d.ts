export interface Crumb { label: string; icon?: string; }
export interface BreadcrumbsProps {
  items: Crumb[];
  /** Index of the clicked crumb */
  onNavigate?: (index: number) => void;
  style?: React.CSSProperties;
}
export declare function Breadcrumbs(props: BreadcrumbsProps): JSX.Element;

export interface SidebarItemProps {
  icon?: string;
  label: string;
  active?: boolean;
  count?: number | string;
  /** Default var(--primary) */
  iconColor?: string;
  onClick?: () => void;
  style?: React.CSSProperties;
}
export declare function SidebarItem(props: SidebarItemProps): JSX.Element;
export interface SidebarSectionProps { title?: string; children?: React.ReactNode; }
export declare function SidebarSection(props: SidebarSectionProps): JSX.Element;

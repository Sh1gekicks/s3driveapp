export interface MenuItem { label?: string; icon?: string; shortcut?: string; destructive?: boolean; disabled?: boolean; separator?: boolean; id?: string; }
export interface MenuProps {
  items: MenuItem[];
  onSelect?: (item: MenuItem) => void;
  style?: React.CSSProperties;
}
export declare function Menu(props: MenuProps): JSX.Element;

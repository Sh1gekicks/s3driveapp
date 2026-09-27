/**
 * @startingPoint section="Shell" subtitle="macOS window — transparent titlebar, vibrant sidebar, toolbar, inspector" viewport="900x520"
 */
export interface AppWindowProps {
  /** Translucent source list; titlebar area (52px) is reserved at its top for traffic lights */
  sidebar?: React.ReactNode;
  /** Unified toolbar living in the transparent titlebar (drag region) */
  toolbar?: React.ReactNode;
  /** Right-hand inspector panel */
  inspector?: React.ReactNode;
  children?: React.ReactNode;
  width?: number | string;
  height?: number | string;
  style?: React.CSSProperties;
}
export declare function AppWindow(props: AppWindowProps): JSX.Element;
export declare function TrafficLights(props: { style?: React.CSSProperties }): JSX.Element;

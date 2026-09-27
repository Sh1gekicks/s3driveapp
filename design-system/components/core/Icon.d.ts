export interface IconProps extends React.SVGProps<SVGSVGElement> {
  /** Lucide icon name, kebab-case (e.g. "folder-plus", "trash-2") */
  name: string;
  /** px. 16 in toolbars/rows, 14 in small buttons, 12 in badges */
  size?: number;
  /** Default 1.75 */
  strokeWidth?: number;
  color?: string;
  /** Tint fill, e.g. for folder glyphs */
  fill?: string;
}
export declare function Icon(props: IconProps): JSX.Element;

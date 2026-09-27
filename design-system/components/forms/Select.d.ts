export interface SelectOption { value: string; label: string; }
export interface SelectProps extends Omit<React.SelectHTMLAttributes<HTMLSelectElement>, 'size'> {
  options: Array<string | SelectOption>;
  label?: string;
  size?: 'sm' | 'md';
}
export declare function Select(props: SelectProps): JSX.Element;

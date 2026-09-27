/**
 * @startingPoint section="Forms" subtitle="Text field with label, icon, hint and error" viewport="700x300"
 */
export interface InputProps extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'size'> {
  /** Leading Lucide icon (e.g. "search") */
  icon?: string;
  label?: string;
  /** Helper or error text under the field */
  hint?: string;
  invalid?: boolean;
  size?: 'sm' | 'md';
}
export declare function Input(props: InputProps): JSX.Element;

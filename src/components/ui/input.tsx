import { Input as BaseInput } from '@base-ui/react/input';
import type { LucideIcon } from 'lucide-react';
import type * as React from 'react';
import { Icon } from '@/components/ds/icon';
import { cn } from '@/lib/utils';

// DS: components/forms/Input.jsx と .s3-input。高さ 28（sm 24）、ヘアラインの枠、背景 --field-bg、影 --shadow-xs。
// フォーカス時は枠も --ring。入力文字は選択できる（02 §6、§7.3）。
// 入力するのは名前やキーなので、OS の自動の大文字化・修正・スペルチェックを既定で止める（02 §6）。
// WKWebView は macOS の「文頭を自動的に大文字にする」を入力欄にも適用するため。

export interface InputProps extends Omit<React.ComponentProps<typeof BaseInput>, 'size'> {
  icon?: LucideIcon;
  size?: 'sm' | 'md';
  invalid?: boolean;
  mono?: boolean;
  wrapperClassName?: string;
}

export function Input({
  icon,
  size = 'md',
  invalid,
  mono,
  className,
  wrapperClassName,
  ...props
}: InputProps) {
  return (
    <div className={cn('relative flex min-w-0 items-center', wrapperClassName)}>
      {icon ? (
        <span className="pointer-events-none absolute left-2 flex text-muted-foreground">
          <Icon icon={icon} size={size === 'sm' ? 12 : 14} />
        </span>
      ) : null}
      <BaseInput
        autoCapitalize="off"
        autoCorrect="off"
        spellCheck={false}
        aria-invalid={invalid || undefined}
        className={cn(
          'h-(--control-h-md) w-full min-w-0 rounded-md border-[0.5px] border-input bg-field px-2 text-base leading-none text-foreground elevation-xs outline-none',
          'transition-[box-shadow,border-color] duration-(--dur-fast) placeholder:text-muted-foreground',
          'focus:border-ring focus:shadow-[0_0_0_3px_var(--ring-soft)] disabled:opacity-50',
          'aria-invalid:border-destructive aria-invalid:shadow-[0_0_0_3px_color-mix(in_oklch,var(--destructive)_20%,transparent)]',
          size === 'sm' && 'h-(--control-h-sm) rounded-sm text-sm',
          icon && (size === 'sm' ? 'pl-6' : 'pl-7'),
          mono && 'font-mono text-sm',
          className,
        )}
        {...props}
      />
    </div>
  );
}

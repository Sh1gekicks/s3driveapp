import { ChevronsUpDown } from 'lucide-react';
import type * as React from 'react';
import { Icon } from '@/components/ds/icon';
import { cn } from '@/lib/utils';

// DS: components/forms/Select.jsx。macOS のポップアップボタン（ネイティブの select ＋上下シェブロン）。
// WKWebView ではネイティブのメニューが開くため、Base UI の Select ではなく native-select を使う（02 §6）。

export interface NativeSelectOption {
  value: string;
  label: string;
}

export interface NativeSelectProps extends Omit<React.ComponentProps<'select'>, 'size'> {
  options: NativeSelectOption[];
  size?: 'sm' | 'md';
  wrapperClassName?: string;
}

export function NativeSelect({
  options,
  size = 'md',
  className,
  wrapperClassName,
  ...props
}: NativeSelectProps) {
  return (
    <div className={cn('relative flex min-w-0 items-center', wrapperClassName)}>
      <select
        className={cn(
          'h-(--control-h-md) w-full min-w-0 appearance-none rounded-md border-[0.5px] border-input bg-field py-0 pr-6.5 pl-2 text-base text-foreground elevation-xs outline-none select-none',
          'transition-[box-shadow,border-color] duration-(--dur-fast) focus:border-ring focus:shadow-[0_0_0_3px_var(--ring-soft)] disabled:opacity-50',
          size === 'sm' && 'h-(--control-h-sm) rounded-sm text-sm',
          className,
        )}
        {...props}
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      <span className="pointer-events-none absolute right-1.75 flex text-muted-foreground">
        <Icon icon={ChevronsUpDown} size={12} />
      </span>
    </div>
  );
}

import { Checkbox as BaseCheckbox } from '@base-ui/react/checkbox';
import { Check, Minus } from 'lucide-react';
import type * as React from 'react';
import { Icon } from '@/components/ds/icon';
import { cn } from '@/lib/utils';

// DS: components/forms/Checkbox.jsx。14px、未確定状態あり（02 §6）。

export interface CheckboxProps {
  checked: boolean;
  indeterminate?: boolean;
  onCheckedChange: (checked: boolean) => void;
  label?: React.ReactNode;
  disabled?: boolean;
  className?: string;
}

export function Checkbox({
  checked,
  indeterminate,
  onCheckedChange,
  label,
  disabled,
  className,
}: CheckboxProps) {
  return (
    // biome-ignore lint/a11y/noLabelWithoutControl: Base UI の Checkbox.Root（role="checkbox" のボタン）をラベルで包んで関連付ける
    <label className={cn('inline-flex items-center gap-2 text-base', disabled && 'opacity-50', className)}>
      <BaseCheckbox.Root
        checked={checked}
        indeterminate={indeterminate}
        disabled={disabled}
        onCheckedChange={(v) => onCheckedChange(Boolean(v))}
        className={cn(
          'flex size-3.5 shrink-0 items-center justify-center rounded-xs border-[0.5px] border-input bg-field text-white elevation-xs outline-none',
          'transition-colors duration-(--dur-fast) focus-visible:shadow-[0_0_0_3px_var(--ring-soft)]',
          'data-checked:border-primary data-checked:bg-primary data-indeterminate:border-primary data-indeterminate:bg-primary',
        )}
      >
        <BaseCheckbox.Indicator className="flex">
          <Icon icon={indeterminate ? Minus : Check} size={10} strokeWidth={3} />
        </BaseCheckbox.Indicator>
      </BaseCheckbox.Root>
      {label}
    </label>
  );
}

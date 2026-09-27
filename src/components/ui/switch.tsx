import { Switch as BaseSwitch } from '@base-ui/react/switch';
import type * as React from 'react';
import { cn } from '@/lib/utils';

// DS: components/forms/Switch.jsx。macOS 風トグル。設定行ではラベル左・スイッチ右（02 §6）。

export interface SwitchProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
  'aria-label'?: string;
  id?: string;
}

export function Switch({ checked, onCheckedChange, disabled, ...props }: SwitchProps) {
  return (
    <BaseSwitch.Root
      checked={checked}
      disabled={disabled}
      onCheckedChange={(v) => onCheckedChange(v)}
      className={cn(
        'relative h-4.5 w-8 shrink-0 rounded-[9px] bg-switch-off p-0 outline-none transition-colors duration-(--dur-base) ease-(--ease-out)',
        'focus-visible:shadow-[0_0_0_3px_var(--ring-soft)] data-checked:bg-primary data-disabled:opacity-50',
      )}
      {...props}
    >
      <BaseSwitch.Thumb
        className={cn(
          'absolute top-0.5 left-0.5 size-3.5 rounded-full bg-white shadow-[0_1px_2px_oklch(0_0_0/.25),0_0_0_0.5px_oklch(0_0_0/.06)]',
          'transition-transform duration-(--dur-base) ease-(--ease-out) data-checked:translate-x-3.5',
        )}
      />
    </BaseSwitch.Root>
  );
}

/** 設定行（ラベル左・スイッチ右）。 */
export function SwitchRow({
  label,
  description,
  ...props
}: SwitchProps & { label: React.ReactNode; description?: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3 text-base">
      <div className="flex min-w-0 flex-col gap-0.5">
        <span>{label}</span>
        {description ? <span className="text-xs text-muted-foreground">{description}</span> : null}
      </div>
      <Switch aria-label={typeof label === 'string' ? label : undefined} {...props} />
    </div>
  );
}

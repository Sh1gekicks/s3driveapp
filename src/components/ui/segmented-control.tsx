import { Toggle } from '@base-ui/react/toggle';
import { ToggleGroup } from '@base-ui/react/toggle-group';
import type { LucideIcon } from 'lucide-react';
import { Icon } from '@/components/ds/icon';
import { cn } from '@/lib/utils';
import { Tooltip } from './tooltip';

// DS: components/forms/SegmentedControl.jsx。表示切り替え、インスペクタのタブ。block で等幅（02 §6）。

export interface SegmentOption<T extends string> {
  value: T;
  label: string;
  icon?: LucideIcon;
  iconOnly?: boolean;
  shortcut?: string;
}

export interface SegmentedControlProps<T extends string> {
  value: T;
  onChange: (value: T) => void;
  options: SegmentOption<T>[];
  block?: boolean;
  className?: string;
  'aria-label'?: string;
}

export function SegmentedControl<T extends string>({
  value,
  onChange,
  options,
  block,
  className,
  ...props
}: SegmentedControlProps<T>) {
  return (
    <ToggleGroup
      value={[value]}
      onValueChange={(v) => {
        const next = v[0] as T | undefined;
        if (next !== undefined) onChange(next);
      }}
      className={cn(
        'inline-flex gap-0.5 rounded-[7px] bg-muted p-0.5 shadow-[inset_0_0_0_0.5px_var(--border)]',
        block && 'flex w-full',
        className,
      )}
      {...props}
    >
      {options.map((o) => {
        const item = (
          <Toggle
            key={o.value}
            value={o.value}
            aria-label={o.label}
            className={cn(
              'inline-flex h-5.5 min-w-7 items-center justify-center gap-1.25 rounded-[5px] px-2.5 text-sm leading-none font-medium text-muted-foreground outline-none',
              'transition-[background-color,color] duration-(--dur-fast) hover:text-foreground focus-visible:shadow-[0_0_0_3px_var(--ring-soft)]',
              'data-pressed:bg-seg-active data-pressed:text-foreground data-pressed:elevation-sm',
              o.iconOnly && 'px-2',
              block && 'flex-1',
            )}
          >
            {o.icon ? <Icon icon={o.icon} size={14} /> : null}
            {o.iconOnly ? null : o.label}
          </Toggle>
        );
        return o.iconOnly ? (
          <Tooltip key={o.value} label={o.label} shortcut={o.shortcut}>
            {item}
          </Tooltip>
        ) : (
          item
        );
      })}
    </ToggleGroup>
  );
}

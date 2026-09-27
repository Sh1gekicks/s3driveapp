import type { StorageClass } from '@/lib/ipc';
import { classColor, classLabel } from '@/lib/storage-class';
import { cn } from '@/lib/utils';

// DS: components/feedback/StorageClassBadge.jsx。ドット（--sc-*）＋名前。short（短い名前）、plain（表のセル用、背景なし）。

export interface StorageClassBadgeProps {
  value: StorageClass;
  short?: boolean;
  plain?: boolean;
  className?: string;
}

export function StorageClassBadge({ value, short = true, plain, className }: StorageClassBadgeProps) {
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 whitespace-nowrap',
        plain
          ? 'text-sm font-normal'
          : 'h-5 rounded-sm bg-muted px-1.75 text-xs leading-none font-medium text-foreground',
        className,
      )}
    >
      <span className="size-1.75 shrink-0 rounded-full" style={{ background: classColor(value) }} />
      {classLabel(value, short)}
    </span>
  );
}

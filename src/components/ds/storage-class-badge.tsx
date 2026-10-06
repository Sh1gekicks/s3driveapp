import type { StorageClass } from '@/lib/ipc';
import { classColor, classLabel } from '@/lib/storage-class';
import { cn } from '@/lib/utils';

// DS: components/feedback/StorageClassBadge.jsx。ドット（--sc-*）＋名前。short（短い名前）、plain（表のセル用、背景なし）、
// truncate（幅に収まらない名前の末尾を省略する）。

export interface StorageClassBadgeProps {
  value: StorageClass;
  short?: boolean;
  plain?: boolean;
  truncate?: boolean;
  className?: string;
}

export function StorageClassBadge({
  value,
  short = true,
  plain,
  truncate,
  className,
}: StorageClassBadgeProps) {
  const label = classLabel(value, short);
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 whitespace-nowrap',
        plain
          ? 'text-sm font-normal'
          : 'h-5 rounded-sm bg-muted px-1.75 text-xs leading-none font-medium text-foreground',
        truncate && 'max-w-full',
        className,
      )}
    >
      <span className="size-1.75 shrink-0 rounded-full" style={{ background: classColor(value) }} />
      {truncate ? <span className="truncate">{label}</span> : label}
    </span>
  );
}

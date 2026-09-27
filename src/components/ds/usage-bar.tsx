import type { StorageClass } from '@/lib/ipc';
import { classColor } from '@/lib/storage-class';

// クラス別の積み上げバー（DS: Progress の segments。02 §6）。

export interface UsageBarProps {
  byClass: Partial<Record<StorageClass, number>>;
  height?: number;
  label?: string;
}

const ORDER: StorageClass[] = [
  'STANDARD',
  'INTELLIGENT_TIERING',
  'STANDARD_IA',
  'ONEZONE_IA',
  'GLACIER_IR',
  'GLACIER',
  'DEEP_ARCHIVE',
  'OTHER',
];

export function UsageBar({ byClass, height = 5, label }: UsageBarProps) {
  const total = ORDER.reduce((s, k) => s + (byClass[k] ?? 0), 0);
  return (
    <div
      role="img"
      aria-label={label}
      className="flex w-full overflow-hidden rounded-full bg-muted shadow-[inset_0_0_0_0.5px_var(--border)]"
      style={{ height }}
    >
      {total > 0
        ? ORDER.filter((k) => (byClass[k] ?? 0) > 0).map((k) => (
            <span
              key={k}
              className="h-full transition-[width] duration-(--dur-slow) ease-(--ease-out)"
              style={{ width: `${((byClass[k] ?? 0) / total) * 100}%`, background: classColor(k) }}
            />
          ))
        : null}
    </div>
  );
}

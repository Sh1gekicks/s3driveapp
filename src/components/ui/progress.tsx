import { Progress as BaseProgress } from '@base-ui/react/progress';
import { cn } from '@/lib/utils';

// DS: components/feedback/Progress.jsx。転送の進捗（高さ 6）。積み上げは ds/usage-bar（02 §6）。

export interface ProgressProps {
  value: number | null;
  max?: number;
  height?: number;
  className?: string;
  'aria-label'?: string;
}

export function Progress({ value, max = 100, height = 6, className, ...props }: ProgressProps) {
  return (
    <BaseProgress.Root value={value} max={max} className={cn('w-full', className)} {...props}>
      <BaseProgress.Track
        className="flex w-full overflow-hidden rounded-full bg-muted shadow-[inset_0_0_0_0.5px_var(--border)]"
        style={{ height }}
      >
        <BaseProgress.Indicator className="h-full bg-primary transition-[width] duration-(--dur-slow) ease-(--ease-out)" />
      </BaseProgress.Track>
    </BaseProgress.Root>
  );
}

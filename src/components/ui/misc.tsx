import { LoaderCircle, type LucideIcon } from 'lucide-react';
import type * as React from 'react';
import { Icon } from '@/components/ds/icon';
import { cn } from '@/lib/utils';

/** 読み込み中（ボタン内など）。 */
export function Spinner({ size = 14, className }: { size?: number; className?: string }) {
  return <Icon icon={LoaderCircle} size={size} className={cn('animate-spin', className)} />;
}

/** 読み込み中の一覧の行など。 */
export function Skeleton({ className, ...props }: React.ComponentProps<'div'>) {
  return <div className={cn('animate-pulse rounded-md bg-muted', className)} {...props} />;
}

/** 空状態（アイコン 32px ＋一言。02 §9.1）。 */
export function Empty({
  icon,
  children,
  action,
  className,
}: {
  icon: LucideIcon;
  children: React.ReactNode;
  action?: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        'grid h-full place-items-center p-6 text-center text-base text-muted-foreground',
        className,
      )}
    >
      <div className="flex flex-col items-center gap-2">
        <Icon icon={icon} size={32} strokeWidth={1.25} />
        <div>{children}</div>
        {action}
      </div>
    </div>
  );
}

/** 0.5px の区切り線。 */
export function Separator({ vertical, className }: { vertical?: boolean; className?: string }) {
  return (
    <span
      role="separator"
      aria-orientation={vertical ? 'vertical' : 'horizontal'}
      className={cn(
        'block shrink-0 bg-border',
        vertical ? 'mx-0.5 h-4.5 w-(--hairline)' : 'h-(--hairline) w-full',
        className,
      )}
    />
  );
}

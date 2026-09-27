import { cva, type VariantProps } from 'class-variance-authority';
import type { LucideIcon } from 'lucide-react';
import type * as React from 'react';
import { Icon } from '@/components/ds/icon';
import { cn } from '@/lib/utils';

// DS: components/feedback/Badge.jsx。高さ 18px。ステータス色の文字は color-mix で foreground に寄せる（02 §3.1）。
export const badgeVariants = cva(
  'inline-flex h-4.5 shrink-0 items-center gap-1 rounded-sm border-[0.5px] border-transparent px-1.5 text-xs leading-none font-medium whitespace-nowrap',
  {
    variants: {
      variant: {
        default: 'bg-primary text-primary-foreground',
        secondary: 'bg-muted text-foreground',
        outline: 'border-border text-foreground',
        success:
          'bg-[color-mix(in_oklch,var(--success)_15%,transparent)] text-[color-mix(in_oklch,var(--success)_65%,var(--foreground))]',
        warning:
          'bg-[color-mix(in_oklch,var(--warning)_18%,transparent)] text-[color-mix(in_oklch,var(--warning)_55%,var(--foreground))]',
        destructive:
          'bg-[color-mix(in_oklch,var(--destructive)_14%,transparent)] text-[color-mix(in_oklch,var(--destructive)_75%,var(--foreground))]',
      },
    },
    defaultVariants: { variant: 'default' },
  },
);

export interface BadgeProps extends React.ComponentProps<'span'>, VariantProps<typeof badgeVariants> {
  icon?: LucideIcon;
}

export function Badge({ className, variant, icon, children, ...props }: BadgeProps) {
  return (
    <span className={cn(badgeVariants({ variant }), className)} {...props}>
      {icon ? <Icon icon={icon} size={11} /> : null}
      {children}
    </span>
  );
}

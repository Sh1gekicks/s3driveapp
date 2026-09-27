import type { LucideIcon } from 'lucide-react';
import type * as React from 'react';
import { cn } from '@/lib/utils';
import { Icon } from './icon';

// DS: components/navigation/SidebarItem.jsx。行 28px、見出し 11px semibold・muted（02 §6）。

export function SidebarSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="flex flex-col gap-px px-2.5" aria-label={title}>
      <h2 className="m-0 px-2 pt-3.5 pb-1.25 text-xs font-semibold text-muted-foreground">{title}</h2>
      {children}
    </section>
  );
}

export interface SidebarItemProps extends Omit<React.ComponentProps<'button'>, 'children'> {
  icon: LucideIcon;
  label: string;
  active?: boolean;
  iconColor?: string;
  count?: React.ReactNode;
}

export function SidebarItem({
  icon,
  label,
  active,
  iconColor,
  count,
  className,
  ...props
}: SidebarItemProps) {
  return (
    <button
      type="button"
      data-active={active ? '' : undefined}
      aria-current={active ? 'page' : undefined}
      className={cn(
        'flex h-7 w-full items-center gap-2 rounded-md px-2 text-left text-base leading-none text-sidebar-foreground outline-none',
        'hover:bg-sidebar-accent focus-visible:shadow-[0_0_0_3px_var(--ring-soft)] data-active:bg-sidebar-accent-strong data-active:font-medium',
        className,
      )}
      {...props}
    >
      <Icon icon={icon} size={16} style={{ color: iconColor ?? 'var(--primary)' }} />
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {count != null ? <span className="text-sm text-muted-foreground tabular-nums">{count}</span> : null}
    </button>
  );
}

import { ChevronRight, Database } from 'lucide-react';
import { Fragment } from 'react';
import { Icon } from '@/components/ds/icon';
import { cn } from '@/lib/utils';

// DS: components/navigation/Breadcrumbs.jsx。先頭はバケット（database アイコン）、最後が現在のフォルダ。
// 幅が足りないときは中間を省略記号にまとめる（02 §6）。

export interface Crumb {
  label: string;
}

export interface BreadcrumbsProps {
  items: Crumb[];
  onNavigate: (index: number) => void;
  /** これを超える階層は中間を「…」にまとめる。 */
  maxItems?: number;
  className?: string;
}

export function Breadcrumbs({ items, onNavigate, maxItems = 4, className }: BreadcrumbsProps) {
  const last = items.length - 1;
  const collapsed = items.length > maxItems;
  const visible = collapsed
    ? [0, -1, ...items.map((_, i) => i).slice(items.length - (maxItems - 2))]
    : items.map((_, i) => i);
  return (
    <nav aria-label="パンくずリスト" className={cn('flex min-w-0 items-center gap-0.5', className)}>
      {visible.map((index, n) => {
        const item = index >= 0 ? items[index] : null;
        return (
          <Fragment key={index < 0 ? 'ellipsis' : `${index}-${item?.label}`}>
            {n > 0 ? <Icon icon={ChevronRight} size={12} className="shrink-0 text-muted-foreground" /> : null}
            {item ? (
              <button
                type="button"
                data-current={index === last ? '' : undefined}
                aria-current={index === last ? 'page' : undefined}
                onClick={() => onNavigate(index)}
                title={item.label}
                className={cn(
                  'inline-flex h-6 min-w-0 items-center gap-1.25 rounded-sm px-1.5 text-base leading-none font-medium whitespace-nowrap text-muted-foreground outline-none',
                  'hover:bg-accent hover:text-foreground focus-visible:shadow-[0_0_0_3px_var(--ring-soft)]',
                  'data-current:font-semibold data-current:text-foreground',
                  index !== 0 && index !== last && 'max-w-40',
                  // 現在のフォルダ名はほかの階層より優先して表示する
                  index === last && 'shrink-[0.2]',
                )}
              >
                {index === 0 ? <Icon icon={Database} size={14} className="shrink-0" /> : null}
                <span className="truncate">{item.label}</span>
              </button>
            ) : (
              <span className="px-1 text-muted-foreground">…</span>
            )}
          </Fragment>
        );
      })}
    </nav>
  );
}

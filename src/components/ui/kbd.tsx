import type * as React from 'react';
import { cn } from '@/lib/utils';

/** キーボードショートカットの表記（⌘ ⇧ ⌥ ⌫）。 */
export function Kbd({ className, ...props }: React.ComponentProps<'kbd'>) {
  return (
    <kbd className={cn('font-sans text-sm tracking-wide text-muted-foreground', className)} {...props} />
  );
}

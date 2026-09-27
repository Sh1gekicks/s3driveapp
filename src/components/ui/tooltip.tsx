import { Tooltip as BaseTooltip } from '@base-ui/react/tooltip';
import type * as React from 'react';
import { cn } from '@/lib/utils';
import { Kbd } from './kbd';

// DS: components/feedback/Tooltip.jsx。暗い小さなラベル。ショートカットがあれば併記する（02 §6）。

export function TooltipProvider({ children }: { children: React.ReactNode }) {
  return (
    <BaseTooltip.Provider delay={500} closeDelay={0}>
      {children}
    </BaseTooltip.Provider>
  );
}

export interface TooltipProps {
  label: React.ReactNode;
  shortcut?: string;
  side?: 'top' | 'bottom' | 'left' | 'right';
  /** ツールチップを付ける要素（1 つの React 要素）。 */
  children: React.ReactElement<Record<string, unknown>>;
}

export function Tooltip({ label, shortcut, side = 'bottom', children }: TooltipProps) {
  return (
    <BaseTooltip.Root>
      <BaseTooltip.Trigger render={children} />
      <BaseTooltip.Portal>
        <BaseTooltip.Positioner side={side} sideOffset={6} className="z-60">
          <BaseTooltip.Popup
            className={cn(
              'pointer-events-none flex items-center gap-1.5 rounded-sm bg-tooltip px-2 py-1 text-xs leading-[1.3] whitespace-nowrap text-tooltip-foreground elevation-sm',
              'transition-opacity duration-(--dur-fast) data-ending-style:opacity-0 data-starting-style:opacity-0',
            )}
          >
            {label}
            {shortcut ? <Kbd className="text-tooltip-foreground/70">{shortcut}</Kbd> : null}
          </BaseTooltip.Popup>
        </BaseTooltip.Positioner>
      </BaseTooltip.Portal>
    </BaseTooltip.Root>
  );
}

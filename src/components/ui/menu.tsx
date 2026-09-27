import { ContextMenu as BaseContextMenu } from '@base-ui/react/context-menu';
import { Menu as BaseMenu } from '@base-ui/react/menu';
import type { LucideIcon } from 'lucide-react';
import type * as React from 'react';
import { Icon } from '@/components/ds/icon';
import { cn } from '@/lib/utils';
import { Kbd } from './kbd';

// DS: components/navigation/Menu.jsx。幅 220、マテリアル。ホバーで primary 塗り＋白文字（macOS 準拠）。
// ショートカット表示、破壊的項目（02 §6）。コンテキストメニュー（MNU-01／02）とアカウントメニュー（MNU-03）に使う。

export type MenuEntry =
  | {
      id: string;
      label: string;
      icon?: LucideIcon;
      shortcut?: string;
      destructive?: boolean;
      disabled?: boolean;
    }
  | { separator: true; id: string };

const popupClass = cn(
  'material min-w-[220px] rounded-lg p-[5px] text-popover-foreground elevation-md outline-none',
  'flex flex-col transition-opacity duration-(--dur-fast) data-ending-style:opacity-0 data-starting-style:opacity-0',
);

const itemClass = cn(
  'flex h-6 items-center gap-2 rounded-xs px-2 text-base leading-none outline-none select-none',
  'data-disabled:opacity-40 data-highlighted:bg-primary data-highlighted:text-primary-foreground',
  '[&[data-highlighted]_kbd]:text-inherit [&[data-highlighted]_kbd]:opacity-80',
);

function Items({
  items,
  onSelect,
  Parts,
}: {
  items: MenuEntry[];
  onSelect: (id: string) => void;
  Parts: typeof BaseMenu;
}) {
  return items.map((item) =>
    'separator' in item ? (
      <Parts.Separator key={item.id} className="mx-2 my-[5px] h-(--hairline) bg-border" />
    ) : (
      <Parts.Item
        key={item.id}
        disabled={item.disabled}
        onClick={() => onSelect(item.id)}
        className={cn(
          itemClass,
          item.destructive &&
            'text-destructive data-highlighted:bg-destructive data-highlighted:text-white [&_svg]:text-destructive [&[data-highlighted]_svg]:text-white',
        )}
      >
        {item.icon ? <Icon icon={item.icon} size={14} /> : <span className="w-3.5" />}
        <span className="flex-1">{item.label}</span>
        {item.shortcut ? <Kbd className="ml-4">{item.shortcut}</Kbd> : null}
      </Parts.Item>
    ),
  );
}

export interface ContextMenuProps {
  items: MenuEntry[];
  onSelect: (id: string) => void;
  /** 右クリックの対象になる領域。 */
  children: React.ReactNode;
  className?: string;
  onOpenChange?: (open: boolean) => void;
}

/** 右クリックで開くメニュー。項目は開く直前の選択状態から作る。 */
export function ContextMenu({ items, onSelect, children, className, onOpenChange }: ContextMenuProps) {
  return (
    <BaseContextMenu.Root onOpenChange={onOpenChange}>
      <BaseContextMenu.Trigger className={className}>{children}</BaseContextMenu.Trigger>
      <BaseContextMenu.Portal>
        <BaseContextMenu.Positioner className="z-50">
          <BaseContextMenu.Popup className={popupClass}>
            <Items items={items} onSelect={onSelect} Parts={BaseMenu} />
          </BaseContextMenu.Popup>
        </BaseContextMenu.Positioner>
      </BaseContextMenu.Portal>
    </BaseContextMenu.Root>
  );
}

export interface DropdownMenuProps {
  items: MenuEntry[];
  onSelect: (id: string) => void;
  /** メニューを開くボタン（1 つの React 要素）。 */
  trigger: React.ReactElement<Record<string, unknown>>;
  side?: 'top' | 'bottom';
  align?: 'start' | 'end';
}

export function DropdownMenu({
  items,
  onSelect,
  trigger,
  side = 'bottom',
  align = 'start',
}: DropdownMenuProps) {
  return (
    <BaseMenu.Root>
      <BaseMenu.Trigger render={trigger} />
      <BaseMenu.Portal>
        <BaseMenu.Positioner side={side} align={align} sideOffset={4} className="z-50">
          <BaseMenu.Popup className={popupClass}>
            <Items items={items} onSelect={onSelect} Parts={BaseMenu} />
          </BaseMenu.Popup>
        </BaseMenu.Positioner>
      </BaseMenu.Portal>
    </BaseMenu.Root>
  );
}

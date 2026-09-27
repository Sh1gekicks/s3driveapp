import type { LucideIcon } from 'lucide-react';
import { Icon } from '@/components/ds/icon';
import { Button, type ButtonProps } from './button';
import { Tooltip } from './tooltip';

// DS: components/core/IconButton.jsx。label は必須（アクセシブル名とツールチップ）。active で押下状態（02 §6）。

export interface IconButtonProps extends Omit<ButtonProps, 'children' | 'size'> {
  icon: LucideIcon;
  label: string;
  shortcut?: string;
  active?: boolean;
  size?: 'sm' | 'md' | 'lg';
  tooltipSide?: 'top' | 'bottom' | 'left' | 'right';
}

export function IconButton({
  icon,
  label,
  shortcut,
  active,
  size = 'md',
  variant = 'ghost',
  tooltipSide,
  ...props
}: IconButtonProps) {
  const iconSize = size === 'sm' ? 14 : 16;
  return (
    <Tooltip label={label} shortcut={shortcut} side={tooltipSide}>
      <Button
        variant={variant}
        size={size === 'sm' ? 'icon-sm' : size === 'lg' ? 'icon-lg' : 'icon'}
        aria-label={label}
        aria-pressed={active === undefined ? undefined : active}
        data-active={active ? '' : undefined}
        {...props}
      >
        <Icon icon={icon} size={iconSize} />
      </Button>
    </Tooltip>
  );
}

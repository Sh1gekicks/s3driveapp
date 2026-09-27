import type { LucideIcon, LucideProps } from 'lucide-react';

// DS: components/core/Icon.jsx。線幅 1.75、ツールバー・行 16px、小ボタン 14px、バッジ 12px、空状態 32px（02 §8.1）。

export type IconComponent = LucideIcon;

export interface IconProps extends Omit<LucideProps, 'ref'> {
  icon: LucideIcon;
}

export function Icon({ icon: Glyph, size = 16, strokeWidth = 1.75, ...props }: IconProps) {
  return <Glyph size={size} strokeWidth={strokeWidth} absoluteStrokeWidth={false} aria-hidden {...props} />;
}

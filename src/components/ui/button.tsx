import { Button as BaseButton } from '@base-ui/react/button';
import { cva, type VariantProps } from 'class-variance-authority';
import type * as React from 'react';
import { cn } from '@/lib/utils';

// DS: components/core/Button.jsx と components.css の .s3-btn（02 §6）。
// ホバーは背景を 6〜10% 濃く、押下はさらに濃く。拡大縮小しない。カーソルは矢印のまま（REQ-D05）。
export const buttonVariants = cva(
  [
    'inline-flex shrink-0 items-center justify-center gap-1.5 whitespace-nowrap select-none outline-none',
    'h-(--control-h) rounded-md border-[0.5px] border-transparent px-3 text-base leading-none font-medium',
    'transition-[background-color,box-shadow,color] duration-(--dur-fast) ease-(--ease-out)',
    'focus-visible:shadow-[0_0_0_3px_var(--ring-soft)]',
    'disabled:pointer-events-none disabled:opacity-45 data-disabled:pointer-events-none data-disabled:opacity-45',
    '[&_svg]:shrink-0',
  ],
  {
    variants: {
      variant: {
        default:
          'bg-primary text-primary-foreground shadow-[var(--shadow-xs),inset_0_0.5px_0_oklch(1_0_0/.22)] hover:bg-[color-mix(in_oklch,var(--primary)_90%,black)] active:bg-[color-mix(in_oklch,var(--primary)_80%,black)]',
        secondary:
          'bg-secondary text-secondary-foreground hover:bg-[color-mix(in_oklch,var(--secondary)_92%,var(--foreground))] active:bg-[color-mix(in_oklch,var(--secondary)_85%,var(--foreground))]',
        outline:
          'border-input bg-field elevation-xs hover:bg-[color-mix(in_oklch,var(--field-bg)_94%,var(--foreground))] active:bg-[color-mix(in_oklch,var(--field-bg)_88%,var(--foreground))]',
        ghost:
          'bg-transparent hover:bg-accent active:bg-[color-mix(in_oklch,var(--accent),var(--foreground)_6%)] data-active:bg-accent data-active:text-primary',
        destructive:
          'bg-destructive text-white elevation-xs hover:bg-[color-mix(in_oklch,var(--destructive)_90%,black)] active:bg-[color-mix(in_oklch,var(--destructive)_80%,black)]',
        link: 'h-auto border-0 px-0 font-normal text-primary hover:underline',
      },
      size: {
        sm: '[--control-h:var(--control-h-sm)] gap-1 rounded-sm px-2 text-sm',
        md: '[--control-h:var(--control-h-md)]',
        lg: '[--control-h:var(--control-h-lg)] px-4',
        'icon-sm': '[--control-h:var(--control-h-sm)] w-(--control-h) rounded-sm px-0',
        icon: '[--control-h:var(--control-h-md)] w-(--control-h) px-0',
        'icon-lg': '[--control-h:var(--control-h-lg)] w-(--control-h) px-0',
      },
    },
    defaultVariants: { variant: 'default', size: 'md' },
  },
);

export type ButtonProps = React.ComponentProps<typeof BaseButton> & VariantProps<typeof buttonVariants>;

export function Button({ className, variant, size, type = 'button', ...props }: ButtonProps) {
  return <BaseButton type={type} className={cn(buttonVariants({ variant, size }), className)} {...props} />;
}

import { type ClassValue, clsx } from 'clsx';
import { twMerge } from 'tailwind-merge';

/** className を結合し、Tailwind のクラスの衝突を解決する（shadcn/ui の cn）。 */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}

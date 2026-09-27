import { useEffect, useState } from 'react';

/** ウィンドウ幅（01 §5.5 の切り替えに使う）。 */
export function useWindowWidth(): number {
  const [width, setWidth] = useState(() => (typeof window === 'undefined' ? 1300 : window.innerWidth));
  useEffect(() => {
    const onResize = () => setWidth(window.innerWidth);
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, []);
  return width;
}

export type Layout = 'wide' | 'narrow' | 'compact';

/** 1,100 px 以上は標準、900〜1,099 px は narrow、900 px 未満は compact（01 §5.5）。 */
export function layoutFor(width: number): Layout {
  if (width >= 1100) return 'wide';
  if (width >= 900) return 'narrow';
  return 'compact';
}

/** ウィンドウが前面にあるか（選択行の色を切り替える。02 §3）。 */
export function useWindowFocused(): boolean {
  const [focused, setFocused] = useState(() =>
    typeof document === 'undefined' ? true : document.hasFocus(),
  );
  useEffect(() => {
    const on = () => setFocused(true);
    const off = () => setFocused(false);
    window.addEventListener('focus', on);
    window.addEventListener('blur', off);
    return () => {
      window.removeEventListener('focus', on);
      window.removeEventListener('blur', off);
    };
  }, []);
  return focused;
}

// 表示中の接続とフォルダ、戻る／進む（01 §5.2。DS: ui_kits/s3-drive/App.jsx と同じ振る舞い）。

import { create } from 'zustand';

export interface NavState {
  connectionId: string | null;
  prefix: string;
  back: string[];
  forward: string[];
  /** 接続を開く（履歴は消す）。 */
  openConnection: (connectionId: string, prefix?: string) => void;
  /** フォルダへ移動する（戻るの履歴に積む）。 */
  navigate: (prefix: string) => void;
  goBack: () => void;
  goForward: () => void;
  goUp: () => void;
  reset: () => void;
}

export function parentPrefix(prefix: string): string {
  const trimmed = prefix.replace(/\/$/, '');
  const i = trimmed.lastIndexOf('/');
  return i < 0 ? '' : trimmed.slice(0, i + 1);
}

export const useNavStore = create<NavState>((set, get) => ({
  connectionId: null,
  prefix: '',
  back: [],
  forward: [],
  openConnection: (connectionId, prefix = '') => set({ connectionId, prefix, back: [], forward: [] }),
  navigate: (prefix) => {
    const s = get();
    if (prefix === s.prefix) return;
    set({ prefix, back: [...s.back, s.prefix], forward: [] });
  },
  goBack: () => {
    const s = get();
    const prev = s.back[s.back.length - 1];
    if (prev === undefined) return;
    set({ prefix: prev, back: s.back.slice(0, -1), forward: [s.prefix, ...s.forward] });
  },
  goForward: () => {
    const s = get();
    const [next, ...rest] = s.forward;
    if (next === undefined) return;
    set({ prefix: next, back: [...s.back, s.prefix], forward: rest });
  },
  goUp: () => {
    const s = get();
    if (!s.prefix) return;
    s.navigate(parentPrefix(s.prefix));
  },
  reset: () => set({ connectionId: null, prefix: '', back: [], forward: [] }),
}));

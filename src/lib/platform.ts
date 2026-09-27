import { isTauri } from '@tauri-apps/api/core';

/** `pnpm dev:mock`（モックバックエンドで動かすブラウザ表示）。 */
export const IS_MOCK = import.meta.env.MODE === 'mock';

/** Tauri のウィンドウで動いているか（モック・テストでは偽）。 */
export function isNative(): boolean {
  return !IS_MOCK && import.meta.env.MODE !== 'test' && isTauri();
}

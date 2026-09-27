// モックバックエンドを IPC に差し込む（`pnpm dev:mock` と画面テスト。09 §2.4）。

import { invoke } from '@tauri-apps/api/core';
import { clearMocks, mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import type { Selection } from '../types';
import { MockBackend, type MockOptions } from './backend';

let current: MockBackend | null = null;

export function installMock(options: MockOptions = {}): MockBackend {
  clearMocks();
  const backend = new MockBackend(options);
  mockWindows('main');
  mockIPC((cmd, args) => backend.handle(cmd, args as Record<string, unknown>), { shouldMockEvents: true });
  current = backend;
  return backend;
}

export function mockBackend(): MockBackend | null {
  return current;
}

/** ブラウザで選んだファイルをモックの選択として登録する（ネイティブのダイアログの代わり）。 */
export function registerBrowserFiles(files: File[]): Promise<Selection> {
  return invoke<Selection>('mock_register_selection', {
    items: files.map((f) => ({ name: f.name, size: f.size, isDir: false })),
  });
}

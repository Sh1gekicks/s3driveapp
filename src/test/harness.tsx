// 画面テストの共通の準備（09 §2.3〜2.4）。モックバックエンドを IPC に差し込んで描画する。
//
// 使う側のテストファイルで、App が使う queryClient（シングルトン）を作り直せるようにしておくこと:
//   vi.mock('@/app/query-client', () => ({ queryClient: new QueryClient({ ... }) }));

import { QueryClientProvider } from '@tanstack/react-query';
import { type RenderResult, render, screen, within } from '@testing-library/react';
import type userEvent from '@testing-library/user-event';
import type React from 'react';
import { App } from '@/app/App';
import { queryClient } from '@/app/query-client';
import { ToastProvider } from '@/components/ui/toaster';
import { TooltipProvider } from '@/components/ui/tooltip';
import type { Entry } from '@/lib/ipc';
import { installMock } from '@/lib/ipc/mock';
import type { MockBackend, MockOptions } from '@/lib/ipc/mock/backend';
import { useNavStore } from '@/stores/nav';
import { NO_FILTERS, useUiStore } from '@/stores/ui';

export type User = ReturnType<typeof userEvent.setup>;

/** テストごとの初期化（beforeEach で呼ぶ）。 */
export function resetScreen() {
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1024 });
  queryClient.clear();
  useNavStore.getState().reset();
  useUiStore.setState({
    view: 'files',
    viewMode: 'list',
    selection: { keys: [], anchor: null },
    query: '',
    filters: NO_FILTERS,
    filtersOpen: false,
    showDeleted: false,
    inspectorVisible: true,
    dialog: null,
  });
  // jsdom にない API。仮想スクロールが行を描けるよう、要素の寸法を与える
  window.HTMLElement.prototype.scrollTo = () => {};
  Object.defineProperty(window.HTMLElement.prototype, 'offsetHeight', { configurable: true, get: () => 800 });
  Object.defineProperty(window.HTMLElement.prototype, 'offsetWidth', { configurable: true, get: () => 1000 });
  // 寸法は上で固定しているため、変化を知らせる必要はない（何もしない）
  globalThis.ResizeObserver ??= class {
    observe() {
      // 何もしない
    }
    unobserve() {
      // 何もしない
    }
    disconnect() {
      // 何もしない
    }
  } as unknown as typeof ResizeObserver;
}

/** モックバックエンドを差し込み、`ui`（既定はメインウィンドウ）を描画する。 */
export function renderWithMock(
  options: MockOptions & { before?: (b: MockBackend) => void } = {},
  ui: React.ReactNode = <App />,
): RenderResult & { backend: MockBackend } {
  const backend = installMock({ tick: 0, ...options });
  options.before?.(backend);
  const view = render(
    <QueryClientProvider client={queryClient}>
      <TooltipProvider>
        <ToastProvider>{ui}</ToastProvider>
      </TooltipProvider>
    </QueryClientProvider>,
  );
  return { backend, ...view };
}

/** 表示中の一覧の項目。 */
export function entry(name: string): Entry {
  const found = queryClient
    .getQueriesData<{ pages: { entries: Entry[] }[] }>({ queryKey: ['objects'] })
    .flatMap(([, data]) => data?.pages.flatMap((p) => p.entries) ?? [])
    .find((e) => e.name === name);
  if (!found) throw new Error(`${name} が一覧にありません`);
  return found;
}

export function fileList(bucket = 'acme-media-tokyo') {
  return screen.findByRole('listbox', { name: bucket });
}

/** 一覧の項目をクリックして選択する（`meta` で追加選択）。 */
export async function select(user: User, name: string, meta = false) {
  const list = await fileList(currentBucket());
  if (meta) await user.keyboard('{Meta>}');
  await user.pointer({ keys: '[MouseLeft]', target: await within(list).findByText(name) });
  if (meta) await user.keyboard('{/Meta}');
}

function currentBucket(): string {
  const id = useNavStore.getState().connectionId;
  return id?.includes('osaka') ? 'acme-backup-osaka' : 'acme-media-tokyo';
}

export async function openBucket(user: User, bucket: string) {
  const sidebar = await screen.findByRole('navigation', { name: 'サイドバー' });
  await user.click(within(sidebar).getByText(bucket));
  return fileList(bucket);
}

/** トーストの要素（`data-tone` を持つ）。 */
export async function findToast(title: string | RegExp): Promise<HTMLElement> {
  const el = (await screen.findAllByText(title))
    .map((e) => e.closest<HTMLElement>('[data-tone]'))
    .find((e) => e !== null);
  if (!el) throw new Error(`${title} のトーストがありません`);
  return el;
}

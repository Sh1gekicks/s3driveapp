// 画面のテスト（09 §2.4）。モックバックエンドを IPC に差し込み、主要な流れを確かめる。

import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ToastProvider } from '@/components/ui/toaster';
import { TooltipProvider } from '@/components/ui/tooltip';
import { installMock } from '@/lib/ipc/mock';
import type { MockBackend } from '@/lib/ipc/mock/backend';
import { useNavStore } from '@/stores/nav';
import { NO_FILTERS, useUiStore } from '@/stores/ui';
import { App } from './App';

// App が使う queryClient（シングルトン）をテストごとに作り直す
vi.mock('./query-client', () => ({
  queryClient: new QueryClient({
    defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } },
  }),
}));

import { queryClient } from './query-client';

let backend: MockBackend;

function renderApp(options: Parameters<typeof installMock>[0] = {}) {
  backend = installMock({ tick: 0, ...options });
  return render(
    <QueryClientProvider client={queryClient}>
      <TooltipProvider>
        <ToastProvider>
          <App />
        </ToastProvider>
      </TooltipProvider>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
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
    dialog: null,
  });
  // jsdom にない API。仮想スクロールが行を描けるよう、要素の寸法を与える
  window.HTMLElement.prototype.scrollTo = () => {};
  Object.defineProperty(window.HTMLElement.prototype, 'offsetHeight', { configurable: true, get: () => 800 });
  Object.defineProperty(window.HTMLElement.prototype, 'offsetWidth', { configurable: true, get: () => 1000 });
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
});

afterEach(() => {
  vi.useRealTimers();
});

describe('SCR-01 サインイン', () => {
  it('未サインインでは Google でサインインを表示し、サインイン後に接続のフォームに進む', async () => {
    const user = userEvent.setup();
    renderApp({ signedOut: true, noConnections: true });
    const button = await screen.findByRole('button', { name: 'Google でサインイン' });
    await user.click(button);
    expect(await screen.findByText(/でサインイン中/)).toBeInTheDocument();
    expect(screen.getByLabelText('アクセスキー ID')).toBeInTheDocument();
  });

  it('入力の検証エラーを入力欄の下に表示する', async () => {
    const user = userEvent.setup();
    renderApp({ noConnections: true });
    await user.type(await screen.findByLabelText('アクセスキー ID'), 'AKIA123');
    await user.click(screen.getByRole('button', { name: '接続' }));
    expect(
      await screen.findByText('AKIA で始まる 20 文字の英大文字・数字を入力してください'),
    ).toBeInTheDocument();
    expect(backend.calls.some((c) => c.cmd === 'connection_create')).toBe(false);
  });
});

describe('SCR-02 ファイルブラウザ', () => {
  it('サイドバーに接続、一覧にフォルダとファイルを表示する', async () => {
    renderApp();
    const sidebar = await screen.findByRole('navigation', { name: 'サイドバー' });
    expect(within(sidebar).getByText('acme-media-tokyo')).toBeInTheDocument();
    expect(within(sidebar).getByText('acme-backup-osaka')).toBeInTheDocument();
    const list = await screen.findByRole('listbox', { name: 'acme-media-tokyo' });
    expect(within(list).getByText('projects')).toBeInTheDocument();
    expect(within(list).getByText('README.md')).toBeInTheDocument();
  });

  it('ダブルクリックでフォルダを開き、パンくずに表示する', async () => {
    const user = userEvent.setup();
    renderApp();
    const list = await screen.findByRole('listbox', { name: 'acme-media-tokyo' });
    await user.dblClick(within(list).getByText('projects'));
    // 読み込み中は一覧を作り直すため、改めて取得する
    expect(await screen.findByText('brand-guidelines.pdf')).toBeInTheDocument();
    const crumbs = screen.getByRole('navigation', { name: 'パンくずリスト' });
    expect(within(crumbs).getByRole('button', { name: 'projects' })).toHaveAttribute('aria-current', 'page');
  });

  it('クリックで選択し、インスペクタに詳細を表示する', async () => {
    const user = userEvent.setup();
    renderApp();
    const list = await screen.findByRole('listbox', { name: 'acme-media-tokyo' });
    await user.pointer({ keys: '[MouseLeft]', target: within(list).getByText('README.md') });
    const inspector = screen.getByRole('complementary', { name: 'インスペクタ' });
    expect(await within(inspector).findByText('text/markdown')).toBeInTheDocument();
    expect(within(inspector).getByRole('button', { name: 'ダウンロード' })).toBeInTheDocument();
  });

  it('⌘クリックで複数選択し、空白部分のクリックで選択を解除する', async () => {
    const user = userEvent.setup();
    renderApp();
    const list = await screen.findByRole('listbox', { name: 'acme-media-tokyo' });
    await user.pointer({ keys: '[MouseLeft]', target: within(list).getByText('README.md') });
    await user.keyboard('{Meta>}');
    await user.pointer({ keys: '[MouseLeft]', target: within(list).getByText('logo.png') });
    await user.keyboard('{/Meta}');
    expect(useUiStore.getState().selection.keys).toEqual(['README.md', 'logo.png']);

    const scroller = list.closest('.overflow-auto');
    if (!scroller) throw new Error('スクロール領域がありません');
    await user.pointer({ keys: '[MouseLeft]', target: scroller });
    expect(useUiStore.getState().selection.keys).toEqual([]);
  });

  it('新規フォルダを作成すると一覧に追加して選択する', async () => {
    const user = userEvent.setup();
    renderApp();
    await screen.findByRole('listbox', { name: 'acme-media-tokyo' });
    await user.click(screen.getByRole('button', { name: '新規フォルダ' }));
    const dialog = await screen.findByRole('dialog');
    const input = within(dialog).getByLabelText('名前');
    // macOS の自動の大文字化で先頭が大文字にならない
    expect(input).toHaveAttribute('autocapitalize', 'off');
    expect(input).toHaveAttribute('autocorrect', 'off');
    expect(input).toHaveAttribute('spellcheck', 'false');
    await user.clear(input);
    await user.type(input, 'reports');
    await user.click(within(dialog).getByRole('button', { name: '作成' }));
    const list = screen.getByRole('listbox', { name: 'acme-media-tokyo' });
    expect(await within(list).findByText('reports')).toBeInTheDocument();
    expect(useUiStore.getState().selection.keys).toEqual(['reports/']);
    expect(await screen.findByText('「reports」を作成しました')).toBeInTheDocument();
  });

  it('同じ名前のフォルダは作成できない', async () => {
    const user = userEvent.setup();
    renderApp();
    await screen.findByRole('listbox', { name: 'acme-media-tokyo' });
    await user.click(screen.getByRole('button', { name: '新規フォルダ' }));
    const dialog = await screen.findByRole('dialog');
    const input = within(dialog).getByLabelText('名前');
    await user.clear(input);
    await user.type(input, 'photos');
    expect(within(dialog).getByText('同じ名前のフォルダがあります')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: '作成' })).toBeDisabled();
  });

  it('削除の確認で削除すると一覧から消え、トーストを表示する', async () => {
    const user = userEvent.setup();
    renderApp();
    const list = await screen.findByRole('listbox', { name: 'acme-media-tokyo' });
    await user.pointer({ keys: '[MouseLeft]', target: within(list).getByText('logo.png') });
    act(() => useUiStore.getState().openDialog({ type: 'delete', items: [findEntry('logo.png')] }));
    const dialog = await screen.findByRole('alertdialog');
    expect(
      within(dialog).getByText('削除マーカーが作成されます。以前のバージョンからいつでも復元できます。'),
    ).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: '削除' }));
    await waitFor(() => expect(within(list).queryByText('logo.png')).not.toBeInTheDocument());
    expect(await screen.findByText('1 項目を削除しました')).toBeInTheDocument();
  });

  it('検索語を入力するとバケット全体を検索する', async () => {
    const user = userEvent.setup();
    renderApp();
    await screen.findByRole('listbox', { name: 'acme-media-tokyo' });
    await user.type(screen.getByRole('searchbox', { name: '検索' }), 'report');
    expect(await screen.findByText(/acme-media-tokyo 全体を検索中/)).toBeInTheDocument();
    expect(await screen.findByText('report-q3.pdf')).toBeInTheDocument();
    const list = screen.getByRole('listbox', { name: 'acme-media-tokyo' });
    expect(within(list).queryByText('logo.png')).not.toBeInTheDocument();
  });

  it('ストレージとコストでは Cost Explorer に自動で問い合わせない', async () => {
    const user = userEvent.setup();
    renderApp();
    const sidebar = await screen.findByRole('navigation', { name: 'サイドバー' });
    await user.click(within(sidebar).getByText('ストレージとコスト'));
    expect(await screen.findByText('ストレージクラス別')).toBeInTheDocument();
    await waitFor(() => expect(backend.calls.some((c) => c.cmd === 'cost_summary')).toBe(true));
    expect(backend.calls.some((c) => c.cmd === 'cost_refresh')).toBe(false);
  });
});

function findEntry(name: string) {
  const entry = [
    ...(queryClient.getQueriesData<{ pages: { entries: import('@/lib/ipc').Entry[] }[] }>({
      queryKey: ['objects'],
    }) ?? []),
  ]
    .flatMap(([, data]) => data?.pages.flatMap((p) => p.entries) ?? [])
    .find((e) => e.name === name);
  if (!entry) throw new Error(`${name} が一覧にありません`);
  return entry;
}

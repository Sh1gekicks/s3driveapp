// ダイアログ・一覧・インスペクタ・トーストのテスト（09 §2.3）。モックバックエンドを IPC に差し込んで確かめる。

import { QueryClient } from '@tanstack/react-query';
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ColumnWidths } from '@/lib/ipc';
import type { MockBackend } from '@/lib/ipc/mock/backend';
import { useNavStore } from '@/stores/nav';
import { COLUMN_WIDTH, useUiStore } from '@/stores/ui';
import { entry, findToast, fileList as list, openBucket, renderWithMock, resetScreen } from '@/test/harness';

// App が使う queryClient（シングルトン）をテストごとに作り直す
vi.mock('@/app/query-client', () => ({
  queryClient: new QueryClient({
    defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } },
  }),
}));

let backend: MockBackend;

function renderApp(before?: (b: MockBackend) => void) {
  const view = renderWithMock({ before });
  backend = view.backend;
  return view;
}

beforeEach(resetScreen);

describe('DLG-02 削除の確認の文言', () => {
  it('バージョニング有効では削除マーカーの説明と「すべてのバージョンを完全に削除する」を出す', async () => {
    const user = userEvent.setup();
    renderApp();
    await within(await list()).findByText('logo.png');
    act(() => useUiStore.getState().openDialog({ type: 'delete', items: [entry('logo.png')] }));
    const dialog = await screen.findByRole('alertdialog');
    expect(
      within(dialog).getByText('削除マーカーが作成されます。以前のバージョンからいつでも復元できます。'),
    ).toBeInTheDocument();
    const all = within(dialog).getByRole('checkbox', { name: 'すべてのバージョンを完全に削除する' });
    expect(all).not.toBeChecked();
    await user.click(all);
    expect(within(dialog).getByText('この操作は取り消せません。')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: '完全に削除' })).toBeInTheDocument();
  });

  it('バージョニング無効では取り消せない旨だけを出し、チェックボックスを出さない', async () => {
    const user = userEvent.setup();
    renderApp();
    const osaka = await openBucket(user, 'acme-backup-osaka');
    await within(osaka).findByText('config.json');
    act(() => useUiStore.getState().openDialog({ type: 'delete', items: [entry('config.json')] }));
    const dialog = await screen.findByRole('alertdialog');
    expect(within(dialog).getByText('この操作は取り消せません。')).toBeInTheDocument();
    expect(within(dialog).queryByRole('checkbox')).not.toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: '削除' })).toBeInTheDocument();
  });

  it('フォルダを含む場合は配下の件数を示す', async () => {
    renderApp();
    await within(await list()).findByText('photos');
    act(() => useUiStore.getState().openDialog({ type: 'delete', items: [entry('photos')] }));
    const dialog = await screen.findByRole('alertdialog');
    expect(await within(dialog).findByText('フォルダ内の 4 項目も削除されます。')).toBeInTheDocument();
  });
});

describe('DLG-04 ストレージクラスを変更', () => {
  it('現在と同じクラスでは「変更」を押せない', async () => {
    const user = userEvent.setup();
    renderApp();
    await within(await list()).findByText('logo.png');
    act(() => useUiStore.getState().openDialog({ type: 'storageClass', items: [entry('logo.png')] }));
    const dialog = await screen.findByRole('dialog');
    const apply = within(dialog).getByRole('button', { name: '変更' });
    expect(apply).toBeDisabled();
    const radios = within(dialog).getAllByRole('radio');
    expect(radios).toHaveLength(7);
    const other = radios.find((r) => r.getAttribute('aria-checked') === 'false');
    if (!other) throw new Error('選択できるクラスがありません');
    await user.click(other);
    expect(apply).toBeEnabled();
    // バージョニング有効のバケットでは、変更前のバージョンが残る旨を示す（04 §8.3）
    expect(
      within(dialog).getByText('変更前のバージョンは元のクラスのまま残り、料金が発生します。'),
    ).toBeInTheDocument();
  });
});

describe('DLG-03 移動', () => {
  it('移動元のフォルダとその配下、現在の親フォルダは選択できない', async () => {
    const user = userEvent.setup();
    renderApp();
    await within(await list()).findByText('photos');
    act(() => useUiStore.getState().openDialog({ type: 'move', items: [entry('photos')] }));
    const dialog = await screen.findByRole('dialog');
    const tree = within(dialog).getByRole('tree');
    const item = async (name: string) => {
      const el = (await within(tree).findByText(name)).closest('[role="treeitem"]');
      if (!el) throw new Error(`${name} がツリーにありません`);
      return el;
    };
    // ルートは現在の親フォルダ、photos は移動元
    expect(await item('/（ルート）')).toHaveAttribute('aria-disabled', 'true');
    expect(await item('photos')).toHaveAttribute('aria-disabled', 'true');
    const projects = await item('projects');
    expect(projects).not.toHaveAttribute('aria-disabled');

    const submit = within(dialog).getByRole('button', { name: '移動' });
    expect(submit).toBeDisabled();
    await user.click(projects);
    expect(submit).toBeEnabled();
    expect(
      within(dialog).getByText(
        '移動先では新しいバージョン履歴になります。移動元には削除マーカーが残ります。',
      ),
    ).toBeInTheDocument();
  });
});

describe('ファイル一覧の状態（03 §5.4）', () => {
  it('最初のページを読み込み中はスケルトンを表示する', async () => {
    let release = () => {};
    const { container } = renderApp((b) => {
      release = b.holdNext('objects_list_page');
    });
    await waitFor(() => expect(container.querySelector('[aria-busy="true"]')).not.toBeNull());
    act(() => release());
    expect(await within(await list()).findByText('logo.png')).toBeInTheDocument();
    expect(container.querySelector('[aria-busy="true"]')).toBeNull();
  });

  it('空のフォルダ', async () => {
    renderApp();
    await within(await list()).findByText('logo.png');
    act(() => useNavStore.getState().navigate('empty/'));
    expect(await screen.findByText('ファイルをドロップしてアップロード')).toBeInTheDocument();
  });

  it('検索結果なし', async () => {
    const user = userEvent.setup();
    renderApp();
    await within(await list()).findByText('logo.png');
    await user.type(screen.getByRole('searchbox', { name: '検索' }), 'zzz-no-match');
    expect(await screen.findByText('一致する項目はありません')).toBeInTheDocument();
  });

  it('読み込みに失敗したら理由と「再試行」を表示する', async () => {
    const user = userEvent.setup();
    renderApp((b) =>
      b.failNext('objects_list_page', 'ACCESS_DENIED', 'この操作を行う権限がありません（s3:ListBucket）'),
    );
    expect(await screen.findByText('この操作を行う権限がありません（s3:ListBucket）')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '再試行' }));
    expect(await within(await list()).findByText('logo.png')).toBeInTheDocument();
  });
});

describe('インスペクタの 4 つの表示（03 §5.6）', () => {
  const inspector = () => screen.getByRole('complementary', { name: 'インスペクタ' });

  it('何も選択していないときは現在のフォルダ（バケット・リージョン・バージョニング）', async () => {
    // 幅 1,100 px 以上では、選択していなくてもインスペクタを表示する（01 §5.5）
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1200 });
    renderApp();
    await within(await list()).findByText('logo.png');
    expect(within(inspector()).getByText('バケット')).toBeInTheDocument();
    expect(within(inspector()).getByText('アジアパシフィック (東京)')).toBeInTheDocument();
    expect(await within(inspector()).findByText('有効')).toBeInTheDocument();
    expect(within(inspector()).queryByRole('button', { name: '移動…' })).not.toBeInTheDocument();
  });

  it('フォルダを選択したときは項目数と「移動…」「削除」', async () => {
    const user = userEvent.setup();
    renderApp();
    const l = await list();
    await user.pointer({ keys: '[MouseLeft]', target: await within(l).findByText('photos') });
    expect(await within(inspector()).findByText(/4 項目 · /)).toBeInTheDocument();
    expect(within(inspector()).getByRole('button', { name: '移動…' })).toBeInTheDocument();
    expect(within(inspector()).getByRole('button', { name: '削除' })).toBeInTheDocument();
  });

  it('ファイルを 1 つ選択したときは「詳細」と「バージョン (n)」', async () => {
    const user = userEvent.setup();
    renderApp();
    const l = await list();
    await user.pointer({ keys: '[MouseLeft]', target: await within(l).findByText('README.md') });
    expect(await within(inspector()).findByText('バージョン (4)')).toBeInTheDocument();
    expect(within(inspector()).getByText('詳細')).toBeInTheDocument();
    expect(await within(inspector()).findByText('text/markdown')).toBeInTheDocument();
  });

  it('詳細を取得できないときは、スケルトンのままにせず理由と「再試行」を出す', async () => {
    const user = userEvent.setup();
    renderApp((b) =>
      b.failNext('object_head', 'ACCESS_DENIED', 'この操作を行う権限がありません（s3:GetObject）'),
    );
    const l = await list();
    await user.pointer({ keys: '[MouseLeft]', target: await within(l).findByText('README.md') });
    const alert = await within(inspector()).findByRole('alert');
    expect(alert).toHaveTextContent('この操作を行う権限がありません（s3:GetObject）');
    // 作成日・Content-Type・暗号化は「—」
    expect(within(inspector()).getAllByText('—')).toHaveLength(3);
    await user.click(within(alert).getByRole('button', { name: '再試行' }));
    expect(await within(inspector()).findByText('text/markdown')).toBeInTheDocument();
    expect(within(inspector()).queryByRole('alert')).not.toBeInTheDocument();
  });

  it('複数選択したときは件数と一括の操作', async () => {
    const user = userEvent.setup();
    renderApp();
    const l = await list();
    await user.pointer({ keys: '[MouseLeft]', target: await within(l).findByText('README.md') });
    await user.keyboard('{Meta>}');
    await user.pointer({ keys: '[MouseLeft]', target: within(l).getByText('logo.png') });
    await user.keyboard('{/Meta}');
    expect(within(inspector()).getByText('2 項目')).toBeInTheDocument();
    expect(within(inspector()).getByRole('button', { name: 'ストレージクラスを変更…' })).toBeInTheDocument();
  });
});

describe('サイドバーの幅（03 §3）', () => {
  const handle = () => screen.findByRole('separator', { name: 'サイドバーの幅' });
  const width = () =>
    screen.getByRole('complementary', { name: 'サイドバー' }).style.getPropertyValue('--sidebar-w');
  const saved = () =>
    backend.calls
      .filter((c) => c.cmd === 'settings_update')
      .map((c) => (c.args.patch as { view?: { sidebarWidth?: number } }).view?.sidebarWidth);

  it('右端のつまみをドラッグして幅を変え、離したときに保存する（180〜360px に収める）', async () => {
    const user = userEvent.setup();
    renderApp();
    const h = await handle();
    expect(h).toHaveAttribute('aria-valuenow', '220');
    await user.pointer([
      { keys: '[MouseLeft>]', target: h, coords: { clientX: 220 } },
      { coords: { clientX: 300 } },
    ]);
    expect(width()).toBe('300px');
    expect(h).toHaveAttribute('aria-valuenow', '300');
    // ドラッグ中は保存しない
    expect(saved()).toEqual([]);
    await user.pointer([{ coords: { clientX: 900 } }, { keys: '[/MouseLeft]' }]);
    expect(width()).toBe('360px');
    expect(saved()).toEqual([360]);
  });

  it('←→ で 10px ずつ変え、キーを離したときに保存する。ダブルクリックで既定の幅に戻す', async () => {
    const user = userEvent.setup();
    renderApp();
    const h = await handle();
    h.focus();
    await user.keyboard('{ArrowLeft}{ArrowLeft}');
    expect(width()).toBe('200px');
    expect(saved()).toEqual([210, 200]);
    // 押し続けている間は保存しない
    await user.keyboard('{ArrowRight>3}');
    expect(width()).toBe('230px');
    expect(saved()).toEqual([210, 200]);
    await user.keyboard('{/ArrowRight}');
    expect(saved()).toEqual([210, 200, 230]);
    await user.keyboard('{Home}');
    expect(h).toHaveAttribute('aria-valuenow', '180');
    // 最小の幅でさらに狭めても保存し直さない
    await user.keyboard('{ArrowLeft}');
    expect(saved()).toEqual([210, 200, 230, 180]);
    await user.dblClick(h);
    expect(width()).toBe('220px');
    expect(saved()).toEqual([210, 200, 230, 180, 220]);
  });

  it('保存した幅で起動し、広げた分だけ狭い表示に切り替える（01 §5.5）', async () => {
    // 1,200 px のウィンドウでも、サイドバーが 360px ならコンテンツは既定の 1,060 px 相当
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1200 });
    renderApp((b) => {
      b.settings.view.sidebarWidth = 360;
    });
    await within(await list()).findByText('logo.png');
    expect(await handle()).toHaveAttribute('aria-valuenow', '360');
    expect(width()).toBe('360px');
    expect(screen.queryByRole('complementary', { name: 'インスペクタ' })).not.toBeInTheDocument();
  });
});

describe('リストの列の幅（03 §5.4）', () => {
  const handle = (column: string) => screen.findByRole('separator', { name: `${column}の列の幅` });
  /** 見出し行の列の幅（`grid-template-columns`）。 */
  const columns = async () =>
    (await handle('更新日')).closest<HTMLElement>('[style]')?.style.gridTemplateColumns.replaceAll(' ', '');
  const saved = () =>
    backend.calls
      .filter((c) => c.cmd === 'settings_update')
      .map((c) => (c.args.patch as { view?: { columnWidths?: ColumnWidths } }).view?.columnWidths);
  const defaults = COLUMN_WIDTH.default;

  it('列の左端のつまみを左にドラッグして広げ、離したときに保存する（60〜400px に収める）', async () => {
    const user = userEvent.setup();
    renderApp();
    const h = await handle('ストレージクラス');
    expect(h).toHaveAttribute('aria-valuenow', '200');
    await user.pointer([
      { keys: '[MouseLeft>]', target: h, coords: { clientX: 800 } },
      { coords: { clientX: 740 } },
    ]);
    expect(h).toHaveAttribute('aria-valuenow', '260');
    expect(await columns()).toBe('minmax(140px,1fr)124px72px260px');
    // ドラッグ中は保存しない
    expect(saved()).toEqual([]);
    // 右に動かすと狭くなる
    await user.pointer([{ coords: { clientX: 1200 } }, { keys: '[/MouseLeft]' }]);
    expect(h).toHaveAttribute('aria-valuenow', '60');
    expect(saved()).toEqual([{ ...defaults, storageClass: 60 }]);
  });

  it('← で広げ → で狭め（10px ずつ）、キーを離したときに保存する。ダブルクリックで既定の幅に戻す', async () => {
    const user = userEvent.setup();
    renderApp();
    const h = await handle('サイズ');
    h.focus();
    await user.keyboard('{ArrowLeft}');
    expect(h).toHaveAttribute('aria-valuenow', '82');
    await user.keyboard('{ArrowRight}{ArrowRight}');
    expect(h).toHaveAttribute('aria-valuenow', '62');
    expect(saved().map((w) => w?.size)).toEqual([82, 72, 62]);
    await user.keyboard('{End}');
    expect(h).toHaveAttribute('aria-valuenow', '400');
    await user.keyboard('{Home}');
    expect(h).toHaveAttribute('aria-valuenow', '60');
    await user.dblClick(h);
    expect(h).toHaveAttribute('aria-valuenow', '72');
    expect(saved().map((w) => w?.size)).toEqual([82, 72, 62, 400, 60, 72]);
    // ほかの列の幅は変えない
    expect(saved().at(-1)).toEqual(defaults);
  });

  it('保存した幅で起動する。幅が狭いときは「種類」の列とつまみを省く（01 §5.5）', async () => {
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1200 });
    const widths = { modified: 150, size: 90, kind: 70, storageClass: 240 };
    renderApp((b) => {
      b.settings.view.columnWidths = widths;
    });
    await within(await list()).findByText('logo.png');
    expect(await columns()).toBe('minmax(200px,1fr)150px90px70px240px');
    expect(await handle('種類')).toHaveAttribute('aria-valuenow', '70');

    act(() => {
      Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1000 });
      window.dispatchEvent(new Event('resize'));
    });
    await waitFor(() =>
      expect(screen.queryByRole('separator', { name: '種類の列の幅' })).not.toBeInTheDocument(),
    );
    expect(await columns()).toBe('minmax(140px,1fr)150px90px240px');
  });
});

describe('トーストの種類（03 §11）', () => {
  const toastWith = findToast;

  it('操作の完了は success', async () => {
    const user = userEvent.setup();
    renderApp();
    await within(await list()).findByText('logo.png');
    act(() => useUiStore.getState().openDialog({ type: 'delete', items: [entry('logo.png')] }));
    await user.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: '削除' }));
    expect(await toastWith('1 項目を削除しました')).toHaveAttribute('data-tone', 'success');
  });

  it('エラーは destructive で理由を示し、認証エラーには「更新…」を付ける', async () => {
    const user = userEvent.setup();
    renderApp();
    await within(await list()).findByText('logo.png');
    backend.failNext('objects_delete', 'CREDENTIALS_EXPIRED', '認証情報の有効期限が切れました');
    act(() => useUiStore.getState().openDialog({ type: 'delete', items: [entry('logo.png')] }));
    await user.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: '削除' }));
    const toast = await toastWith('削除できませんでした');
    expect(toast).toHaveAttribute('data-tone', 'destructive');
    expect(within(toast).getByText('認証情報の有効期限が切れました')).toBeInTheDocument();
    // Base UI はフォーカスしていないトーストを aria-hidden にするため、hidden も含めて探す
    expect(within(toast).getByRole('button', { name: '更新…', hidden: true })).toBeInTheDocument();
  });
});

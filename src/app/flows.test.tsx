// 主要な操作の流れ（09 §2.4）。モックバックエンドで、メニュー・ショートカット・ダイアログから操作する。

import { QueryClient } from '@tanstack/react-query';
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { signOut, uploadSelection } from '@/features/actions';
import { handleCommand } from '@/features/commands';
import type { CostSummary, TransferJob } from '@/lib/ipc';
import { registerBrowserFiles } from '@/lib/ipc/mock';
import { useNavStore } from '@/stores/nav';
import { useTransferStore } from '@/stores/transfers';
import { useUiStore } from '@/stores/ui';
import { entry, fileList, findToast, renderWithMock, resetScreen, select } from '@/test/harness';

vi.mock('@/app/query-client', () => ({
  queryClient: new QueryClient({
    defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } },
  }),
}));

beforeEach(() => {
  resetScreen();
  useTransferStore.setState({ jobs: {} });
});

/** 転送中のジョブ（サインアウト・アップデートの確認に使う）。 */
function runningTransfer(): TransferJob {
  return {
    jobId: 'job-running',
    kind: 'upload',
    connectionId: 'conn-tokyo',
    title: '1 件をアップロード中',
    status: 'running',
    totalFiles: 1,
    doneFiles: 0,
    failedFiles: 0,
    totalBytes: 1000,
    doneBytes: 100,
    currentName: 'big.bin',
    bytesPerSec: 0,
    etaSec: null,
    destination: 'acme-media-tokyo/',
  };
}

/** タグ Name=s3drive のコスト（0.001 ドル未満）。 */
const TAGGED_COST: CostSummary = {
  scope: { kind: 'tag', key: 'Name', value: 's3drive' },
  month: '2026-09',
  monthToDate: 0.00028738,
  prevMonthSamePeriod: 0,
  breakdown: { storage: 0, requests: 0.00028738, transfer: 0, retrieval: 0, other: 0 },
  daily: Array.from({ length: 30 }, (_, i) => (i < 27 ? 0 : i === 27 ? 0.00028738 : null)),
  forecastMonthEnd: null,
  currency: 'USD',
  fetchedAt: '2026-09-28T23:19:50Z',
};

describe('アップロード（04 §4）', () => {
  it('選んだファイルをアップロードすると一覧に表示され、完了を知らせる', async () => {
    renderWithMock();
    await within(await fileList()).findByText('logo.png');
    const selection = await registerBrowserFiles([new File(['hello'], 'notes.txt')]);
    await act(() => uploadSelection(selection.selectionId, ''));
    expect(await findToast('アップロードが完了しました')).toHaveAttribute('data-tone', 'success');
    expect(await within(await fileList()).findByText('notes.txt')).toBeInTheDocument();
  });

  it('同名の項目があれば DLG-08 で扱いを選ぶ（両方を残す）', async () => {
    const user = userEvent.setup();
    renderWithMock();
    await within(await fileList()).findByText('README.md');
    const selection = await registerBrowserFiles([new File(['# new'], 'README.md')]);
    // 確認が終わるまで戻らないため、待たずに進める
    const uploading = uploadSelection(selection.selectionId, '');
    const dialog = await screen.findByRole('alertdialog');
    expect(within(dialog).getByText('「README.md」はすでに存在します。置き換えますか？')).toBeInTheDocument();
    // バージョニング有効のバケット
    expect(within(dialog).getByText('置き換えても以前のバージョンは残ります。')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: '両方を残す' }));
    await act(() => uploading);
    expect(await within(await fileList()).findByText('README (1).md')).toBeInTheDocument();
  });

  it('除外した項目（.DS_Store）を知らせる', async () => {
    renderWithMock();
    await within(await fileList()).findByText('logo.png');
    const selection = await registerBrowserFiles([new File(['x'], '.DS_Store'), new File(['y'], 'ok.txt')]);
    await act(() => uploadSelection(selection.selectionId, ''));
    expect(await findToast(/項目をアップロードの対象から除外しました/)).toHaveAttribute(
      'data-tone',
      'warning',
    );
  });
});

describe('ダウンロード（04 §5）', () => {
  it('選択した項目をダウンロードし、「Finder に表示」を付けて知らせる', async () => {
    const user = userEvent.setup();
    const { backend } = renderWithMock();
    await select(user, 'README.md');
    act(() => handleCommand('file.download'));
    const toast = await findToast('ダウンロードが完了しました');
    expect(within(toast).getByRole('button', { name: 'Finder に表示', hidden: true })).toBeInTheDocument();
    expect(backend.calls.find((c) => c.cmd === 'download_start')?.args.destination).toBe('default');
  });
});

describe('名前の変更・移動・クラス変更・取り出し', () => {
  it('名前を変更する（DLG-10）', async () => {
    const user = userEvent.setup();
    renderWithMock();
    await select(user, 'logo.png');
    act(() => handleCommand('file.rename'));
    const dialog = await screen.findByRole('dialog');
    const input = within(dialog).getByLabelText('名前');
    await user.clear(input);
    await user.type(input, 'brand.png');
    await user.click(within(dialog).getByRole('button', { name: '変更' }));
    expect(await findToast('名前を変更しました')).toHaveAttribute('data-tone', 'success');
    expect(await within(await fileList()).findByText('brand.png')).toBeInTheDocument();
  });

  it('フォルダへ移動する（DLG-03）', async () => {
    const user = userEvent.setup();
    renderWithMock();
    await select(user, 'logo.png');
    act(() => handleCommand('file.move'));
    const dialog = await screen.findByRole('dialog');
    await user.click(await within(within(dialog).getByRole('tree')).findByText('photos'));
    await user.click(within(dialog).getByRole('button', { name: '移動' }));
    expect(await findToast('1 項目を移動しました')).toHaveAttribute('data-tone', 'success');
    await waitFor(async () => expect(within(await fileList()).queryByText('logo.png')).toBeNull());
  });

  it('ストレージクラスを変更する（DLG-04）', async () => {
    const user = userEvent.setup();
    renderWithMock();
    await select(user, 'logo.png');
    act(() => handleCommand('file.storage_class'));
    const dialog = await screen.findByRole('dialog');
    const target = within(dialog)
      .getAllByRole('radio')
      .find((r) => r.getAttribute('aria-checked') === 'false');
    if (!target) throw new Error('選択できるクラスがありません');
    await user.click(target);
    await user.click(within(dialog).getByRole('button', { name: '変更' }));
    expect(await findToast('ストレージクラスを変更しました')).toHaveAttribute('data-tone', 'success');
  });

  it('アーカイブを取り出す（DLG-07）', async () => {
    const user = userEvent.setup();
    const { backend } = renderWithMock();
    await within(await fileList()).findByText('backups');
    act(() => useNavStore.getState().navigate('backups/'));
    await select(user, 'db-2026-09-01.sql.gz');
    act(() => handleCommand('file.restore'));
    const dialog = await screen.findByRole('dialog');
    expect(
      within(dialog).getByText('取り出しには料金がかかります。完了したら通知します。'),
    ).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: '取り出し' }));
    expect(await findToast('取り出しを開始しました')).toHaveAttribute('data-tone', 'success');
    expect(backend.calls.some((c) => c.cmd === 'objects_request_restore')).toBe(true);
  });
});

describe('バージョン管理（04 §11）', () => {
  it('以前のバージョンを復元し、バージョンを完全に削除する', async () => {
    const user = userEvent.setup();
    renderWithMock();
    await select(user, 'README.md');
    const inspector = screen.getByRole('complementary', { name: 'インスペクタ' });
    await user.click(await within(inspector).findByText('バージョン (4)'));
    const restore = await within(inspector).findAllByRole('button', { name: 'このバージョンを復元' });
    // 最新のバージョンは復元できない
    expect(restore[0]).toBeDisabled();
    await user.click(restore[1] as HTMLElement);
    expect(await findToast('バージョンを復元しました')).toHaveAttribute('data-tone', 'success');

    const remove = await within(inspector).findAllByRole('button', { name: 'このバージョンを完全に削除' });
    await user.click(remove[remove.length - 1] as HTMLElement);
    const dialog = await screen.findByRole('alertdialog');
    expect(within(dialog).getByText('このバージョンを完全に削除しますか？')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: '完全に削除' }));
    expect(await findToast('バージョンを完全に削除しました')).toHaveAttribute('data-tone', 'success');
  });

  it('削除済みの項目を表示して復元する', async () => {
    const user = userEvent.setup();
    renderWithMock();
    await select(user, 'logo.png');
    act(() => useUiStore.getState().openDialog({ type: 'delete', items: [entry('logo.png')] }));
    await user.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: '削除' }));
    await findToast('1 項目を削除しました');
    act(() => handleCommand('view.deleted'));
    await select(user, 'logo.png');
    act(() => useUiStore.getState().setSelection(['logo.png']));
    act(() => handleCommand('file.delete'));
    // 削除済みの項目の「削除」は完全に削除の確認になる
    expect(await screen.findByRole('alertdialog')).toBeInTheDocument();
    act(() => useUiStore.getState().closeDialog());
    const { runSelectionAction } = await import('@/features/actions');
    act(() => runSelectionAction('undelete', [entry('logo.png')]));
    expect(await findToast('1 項目を復元しました')).toHaveAttribute('data-tone', 'success');
  });
});

describe('メニューとショートカット（03 §9〜10）', () => {
  it('表示の切り替え', async () => {
    renderWithMock();
    await within(await fileList()).findByText('logo.png');
    act(() => handleCommand('view.grid'));
    expect(useUiStore.getState().viewMode).toBe('grid');
    act(() => handleCommand('view.list'));
    expect(useUiStore.getState().viewMode).toBe('list');
    act(() => handleCommand('view.filters'));
    expect(await screen.findByText('絞り込み')).toBeInTheDocument();
    act(() => handleCommand('view.inspector'));
    expect(useUiStore.getState().inspectorVisible).toBe(false);
    act(() => handleCommand('view.hidden'));
    await waitFor(() => expect(useUiStore.getState().view).toBe('files'));
  });

  it('移動（戻る・進む・親フォルダ・開く）', async () => {
    const user = userEvent.setup();
    renderWithMock();
    await select(user, 'projects');
    act(() => handleCommand('go.open'));
    expect(useNavStore.getState().prefix).toBe('projects/');
    act(() => handleCommand('go.up'));
    expect(useNavStore.getState().prefix).toBe('');
    act(() => handleCommand('go.back'));
    expect(useNavStore.getState().prefix).toBe('projects/');
    act(() => handleCommand('go.forward'));
    expect(useNavStore.getState().prefix).toBe('');
    act(() => handleCommand('go.dashboard'));
    expect(useUiStore.getState().view).toBe('dashboard');
  });

  it('⌘A ですべてを選択し、⇧⌘N で新規フォルダを開く', async () => {
    const user = userEvent.setup();
    renderWithMock();
    await within(await fileList()).findByText('logo.png');
    // jsdom は macOS ではないため、修飾キーは Control になる
    await user.keyboard('{Control>}a{/Control}');
    expect(useUiStore.getState().selection.keys.length).toBeGreaterThan(3);
    await user.keyboard('{Control>}{Shift>}N{/Shift}{/Control}');
    expect(await screen.findByRole('dialog')).toHaveTextContent('新規フォルダ');
  });

  it('キーをコピーする', async () => {
    const user = userEvent.setup();
    renderWithMock();
    await select(user, 'logo.png');
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });
    act(() => handleCommand('edit.copy_key'));
    expect(await findToast('キーをコピーしました')).toBeInTheDocument();
  });

  it('アップデートの確認（最新の場合）', async () => {
    renderWithMock();
    await within(await fileList()).findByText('logo.png');
    act(() => handleCommand('app.check_update'));
    expect(await findToast('最新のバージョンを使用しています')).toBeInTheDocument();
  });
});

describe('ストレージとコスト（SCR-03）', () => {
  it('「取得」を押したときだけ Cost Explorer に問い合わせる', async () => {
    const user = userEvent.setup();
    const { backend } = renderWithMock();
    await within(await fileList()).findByText('logo.png');
    act(() => handleCommand('go.dashboard'));
    const [fetch] = await screen.findAllByRole('button', { name: '取得' });
    expect(backend.calls.some((c) => c.cmd === 'cost_refresh')).toBe(false);
    await user.click(fetch as HTMLElement);
    await waitFor(() => expect(backend.calls.some((c) => c.cmd === 'cost_refresh')).toBe(true));
    // 再読み込み（⌘R）でも Cost Explorer には問い合わせない
    const before = backend.calls.filter((c) => c.cmd === 'cost_refresh').length;
    act(() => handleCommand('view.reload'));
    await waitFor(() => expect(backend.calls.filter((c) => c.cmd === 'cost_refresh')).toHaveLength(before));
  });

  it('コスト配分タグがなければ、アカウント全体の S3 の金額であることを示す', async () => {
    const user = userEvent.setup();
    renderWithMock();
    await within(await fileList()).findByText('logo.png');
    act(() => handleCommand('go.dashboard'));
    const [fetch] = await screen.findAllByRole('button', { name: '取得' });
    await user.click(fetch as HTMLElement);
    expect(await screen.findByText('アカウント全体の S3')).toBeInTheDocument();
    expect(
      screen.getByText(/ap-northeast-1 の S3 すべて（ほかのバケットを含む）の金額です/),
    ).toBeInTheDocument();
  });

  it('コスト配分タグがあれば、そのタグが付いたバケットの合計として 0.001 ドル未満も示す', async () => {
    renderWithMock({ before: (b) => b.costs.set('conn-tokyo', TAGGED_COST) });
    await within(await fileList()).findByText('logo.png');
    act(() => handleCommand('go.dashboard'));
    expect(await screen.findByText('タグ Name=s3drive')).toBeInTheDocument();
    // KPI・内訳のリクエスト・合計
    expect(screen.getAllByText('$0.00029')).toHaveLength(3);
    expect(screen.getByText(/同じタグのバケットが複数あれば合算します/)).toBeInTheDocument();
    expect(screen.getByText(/^タグ Name=s3drive が付いた S3/)).toBeInTheDocument();
  });

  it('「更新」に失敗したら、前回の結果を表示したままカード内に理由を示す（03 §6）', async () => {
    const user = userEvent.setup();
    const { backend } = renderWithMock({ before: (b) => b.costs.set('conn-tokyo', TAGGED_COST) });
    await within(await fileList()).findByText('logo.png');
    act(() => handleCommand('go.dashboard'));
    expect(await screen.findByText('タグ Name=s3drive')).toBeInTheDocument();
    backend.failNext(
      'cost_refresh',
      'COST_UNAVAILABLE',
      'Cost Explorer にアクセスする権限がありません（ce:GetCostAndUsage）',
    );
    await user.click(screen.getByRole('button', { name: /Cost Explorer の料金/ }));
    expect(await screen.findByText(/コストを更新できませんでした/)).toBeInTheDocument();
    expect(screen.getByText(/ce:GetCostAndUsage/)).toBeInTheDocument();
    expect(screen.getAllByText('$0.00029')).toHaveLength(3);
    // 別の接続を開いたときは表示しない
    act(() => useNavStore.getState().openConnection('conn-osaka'));
    await waitFor(() => expect(screen.queryByText(/コストを更新できませんでした/)).toBeNull());
  });

  it('CloudWatch のメトリクスもインデックスもなければ、インデックスを作成して集計する', async () => {
    const user = userEvent.setup();
    renderWithMock({
      before: (b) => {
        for (const bucket of b.buckets) bucket.usageGb = {};
      },
    });
    await within(await fileList()).findByText('logo.png');
    act(() => handleCommand('go.dashboard'));
    await screen.findByText(/CloudWatch に容量のメトリクスがまだありません/);
    await user.click(screen.getByRole('button', { name: 'インデックスを作成' }));
    expect(await screen.findByText('インデックスから集計（現行バージョンのみ）')).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'インデックスを作成' })).toBeNull();
  });
});

describe('サインアウト（04 §1.4）', () => {
  it('転送中なら確認し、サインアウトすると転送を中止する', async () => {
    const user = userEvent.setup();
    const { backend } = renderWithMock();
    await within(await fileList()).findByText('logo.png');
    act(() => useTransferStore.getState().upsert(runningTransfer()));
    await act(() => signOut());
    const dialog = await screen.findByRole('alertdialog');
    expect(
      within(dialog).getByText('1 件の転送が完了していません。サインアウトすると転送を中止します。'),
    ).toBeInTheDocument();
    expect(backend.calls.some((c) => c.cmd === 'auth_sign_out')).toBe(false);
    await user.click(within(dialog).getByRole('button', { name: 'サインアウト' }));
    await waitFor(() => expect(backend.calls.some((c) => c.cmd === 'auth_sign_out')).toBe(true));
    expect(await screen.findByRole('button', { name: 'Google でサインイン' })).toBeInTheDocument();
  });

  it('転送がなければ確認せずにサインアウトする', async () => {
    const { backend } = renderWithMock();
    await within(await fileList()).findByText('logo.png');
    await act(() => signOut());
    expect(backend.calls.some((c) => c.cmd === 'auth_sign_out')).toBe(true);
    expect(screen.queryByRole('alertdialog')).toBeNull();
  });
});

describe('アップデートの適用（08 §7）', () => {
  it('転送がなければ、すぐに適用して再起動する', async () => {
    const user = userEvent.setup();
    const { backend } = renderWithMock();
    await within(await fileList()).findByText('logo.png');
    act(() => useUiStore.getState().openDialog({ type: 'update', version: '0.3.0', notes: null }));
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('button', { name: 'アップデート' }));
    await waitFor(() =>
      expect(backend.calls.find((c) => c.cmd === 'app_install_update')?.args.whenIdle).toBe(false),
    );
  });

  it('転送中なら、転送の完了後に再起動するかを確認する', async () => {
    const user = userEvent.setup();
    const { backend } = renderWithMock();
    await within(await fileList()).findByText('logo.png');
    act(() => useTransferStore.getState().upsert(runningTransfer()));
    act(() => useUiStore.getState().openDialog({ type: 'update', version: '0.3.0', notes: null }));
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('button', { name: 'アップデート' }));
    expect(within(dialog).getByText(/1 件の転送が完了していません。/)).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: '転送を中止して再起動' })).toBeInTheDocument();
    expect(backend.calls.some((c) => c.cmd === 'app_install_update')).toBe(false);
    await user.click(within(dialog).getByRole('button', { name: '転送の完了後に再起動' }));
    await waitFor(() =>
      expect(backend.calls.find((c) => c.cmd === 'app_install_update')?.args.whenIdle).toBe(true),
    );
    expect(await findToast('アップデートをインストールしました')).toBeInTheDocument();
  });
});

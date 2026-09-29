// 設定ウィンドウ（SCR-04）と接続のダイアログ（DLG-05・06）のテスト（09 §2.3）。

import { QueryClient } from '@tanstack/react-query';
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useUiStore } from '@/stores/ui';
import { fileList, findToast, renderWithMock, resetScreen } from '@/test/harness';
import { SettingsWindow } from './settings-window';

vi.mock('@/app/query-client', () => ({
  queryClient: new QueryClient({
    defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } },
  }),
}));

beforeEach(resetScreen);

// テスト用の架空の値。シークレットスキャンに実在のキーと誤検知されないよう、アクセスキー ID は分けて書く
const KEY_ID = `AKIA${'TESTFAKEKEY00000'}`;
const SECRET = 'test-secret-access-key-for-s3drive-00000';

function renderSettings() {
  return renderWithMock({}, <SettingsWindow />);
}

async function tab(user: ReturnType<typeof userEvent.setup>, name: string) {
  const tabs = await screen.findByRole('group', { name: '設定' });
  await user.click(within(tabs).getByRole('button', { name }));
}

describe('SCR-04 設定', () => {
  it('一般: 変更は即時に保存し、ダウンロード先はパスを渡さずに選ぶ', async () => {
    const user = userEvent.setup();
    const { backend } = renderSettings();
    await user.click(await screen.findByRole('switch', { name: '隠しファイルを表示' }));
    await waitFor(() =>
      expect(backend.calls.find((c) => c.cmd === 'settings_update')?.args.patch).toEqual({
        general: { showHidden: true },
      }),
    );
    await user.click(screen.getByRole('button', { name: '変更…' }));
    await waitFor(() => expect(backend.calls.some((c) => c.cmd === 'app_choose_download_dir')).toBe(true));
    // ダウンロード先のパスは settings_update で送らない（05 §3.9）
    expect(
      backend.calls
        .filter((c) => c.cmd === 'settings_update')
        .every((c) => !JSON.stringify(c.args.patch).includes('downloadDir')),
    ).toBe(true);
  });

  it('転送: 並列数とマルチパートの境界を変更する', async () => {
    const user = userEvent.setup();
    const { backend } = renderSettings();
    await tab(user, '転送');
    await user.selectOptions(await screen.findByLabelText('同時に転送するファイル数'), '5');
    await user.selectOptions(screen.getByLabelText('マルチパートにする最小サイズ'), '32');
    await waitFor(() =>
      expect(backend.settings.transfer).toMatchObject({ maxFiles: 5, multipartThresholdMb: 32 }),
    );
  });

  it('転送: アップロード時のストレージクラスを接続ごとに変える（04 §4.3）', async () => {
    const user = userEvent.setup();
    const { backend } = renderSettings();
    await tab(user, '転送');
    const select = await screen.findByLabelText('acme-media-tokyo アップロード時のストレージクラス');
    // 未設定の接続は全体の設定に従う
    expect(select).toHaveValue('');
    expect(within(select).getByRole('option', { name: '既定（Standard）' })).toBeInTheDocument();
    await user.selectOptions(select, 'STANDARD_IA');
    await waitFor(() =>
      expect(backend.connectionPatches.get('conn-tokyo')).toEqual({ defaultStorageClass: 'STANDARD_IA' }),
    );
    await waitFor(() => expect(select).toHaveValue('STANDARD_IA'));
    // 「既定」に戻すと未設定（null）にする
    await user.selectOptions(select, '');
    await waitFor(() =>
      expect(backend.connectionPatches.get('conn-tokyo')).toEqual({ defaultStorageClass: null }),
    );
  });

  it('接続: AssumeRole の接続だけ SourceIdentity を切り替えられる（04 §2.3）', async () => {
    const user = userEvent.setup();
    const { backend } = renderSettings();
    await tab(user, '接続');
    const toggle = await screen.findByRole('switch', { name: 'acme-media-tokyo' });
    // ロール ARN のない接続には出さない
    expect(screen.queryByRole('switch', { name: 'acme-backup-osaka' })).toBeNull();
    expect(toggle).not.toBeChecked();
    await user.click(toggle);
    await waitFor(() =>
      expect(backend.connectionPatches.get('conn-tokyo')).toEqual({ useSourceIdentity: true }),
    );
    await waitFor(() => expect(toggle).toBeChecked());
  });

  it('接続: 一覧と編集・削除', async () => {
    const user = userEvent.setup();
    const { backend } = renderSettings();
    await tab(user, '接続');
    const list = await screen.findByRole('list');
    expect(within(list).getByText('acme-media-tokyo')).toBeInTheDocument();
    expect(within(list).getByText('acme-backup-osaka')).toBeInTheDocument();
    const [remove] = screen.getAllByRole('button', { name: '削除…' });
    await user.click(remove as HTMLElement);
    const dialog = await screen.findByRole('alertdialog');
    expect(within(dialog).getByText(/への接続を削除しますか？/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: '削除' }));
    await waitFor(() => expect(backend.calls.some((c) => c.cmd === 'connection_delete')).toBe(true));
  });

  it('コスト: Cost Explorer の利用とコスト配分タグ', async () => {
    const user = userEvent.setup();
    const { backend } = renderSettings();
    await tab(user, 'コスト');
    await user.click(await screen.findByRole('switch', { name: /Cost Explorer を使う/ }));
    await waitFor(() => expect(backend.settings.cost.useCostExplorer).toBe(false));
    await user.type(screen.getByLabelText('acme-media-tokyo キー'), 'project');
    await user.type(screen.getByLabelText('acme-media-tokyo 値'), 's3drive');
    await user.tab();
    await waitFor(() => expect(backend.calls.some((c) => c.cmd === 'connection_patch')).toBe(true));
  });

  it('詳細: インデックスの再構築・キャッシュの削除・アップデートの確認', async () => {
    const user = userEvent.setup();
    const { backend } = renderSettings();
    await tab(user, '詳細');
    const [rebuild] = await screen.findAllByRole('button', { name: '再構築' });
    await user.click(rebuild as HTMLElement);
    await waitFor(() => expect(backend.calls.some((c) => c.cmd === 'search_index_rebuild')).toBe(true));
    await user.click(screen.getByRole('button', { name: 'キャッシュを削除' }));
    expect(await findToast('キャッシュを削除しました')).toHaveAttribute('data-tone', 'success');
    await user.selectOptions(screen.getByLabelText('ログレベル'), 'debug');
    await waitFor(() => expect(backend.settings.advanced.logLevel).toBe('debug'));
    await user.click(screen.getByRole('button', { name: '今すぐ確認' }));
    expect(await findToast('最新のバージョンを使用しています')).toBeInTheDocument();
  });
});

describe('DLG-05 バケットを追加', () => {
  it('新しいアクセスキーで接続すると、サイドバーに追加してその接続を開く', async () => {
    const user = userEvent.setup();
    const { backend } = renderWithMock();
    await within(await fileList()).findByText('logo.png');
    act(() => useUiStore.getState().openDialog({ type: 'addBucket' }));
    const dialog = await screen.findByRole('dialog');
    const newKey = within(dialog).queryByRole('radio', { name: '新しいアクセスキー' });
    if (newKey) await user.click(newKey);
    await user.type(within(dialog).getByLabelText('アクセスキー ID'), KEY_ID);
    await user.type(within(dialog).getByLabelText('シークレットアクセスキー'), SECRET);
    await user.type(within(dialog).getByLabelText('バケット名'), 'acme-archive');
    await user.click(within(dialog).getByRole('button', { name: '接続' }));
    await waitFor(() => expect(backend.calls.some((c) => c.cmd === 'connection_create')).toBe(true));
    const sidebar = screen.getByRole('navigation', { name: 'サイドバー' });
    expect(await within(sidebar).findByText('acme-archive')).toBeInTheDocument();
    // 追加した接続を開く（空のバケット）
    expect(await screen.findByText('ファイルをドロップしてアップロード')).toBeInTheDocument();
  });

  it('バケットがない場合はバケット名の下にエラーを表示する', async () => {
    const user = userEvent.setup();
    renderWithMock();
    await within(await fileList()).findByText('logo.png');
    act(() => useUiStore.getState().openDialog({ type: 'addBucket' }));
    const dialog = await screen.findByRole('dialog');
    const newKey = within(dialog).queryByRole('radio', { name: '新しいアクセスキー' });
    if (newKey) await user.click(newKey);
    await user.type(within(dialog).getByLabelText('アクセスキー ID'), KEY_ID);
    await user.type(within(dialog).getByLabelText('シークレットアクセスキー'), SECRET);
    await user.type(within(dialog).getByLabelText('バケット名'), 'missing-bucket');
    await user.click(within(dialog).getByRole('button', { name: '接続' }));
    expect(await within(dialog).findByText('バケットが見つかりません')).toBeInTheDocument();
  });
});

describe('DLG-06 認証情報を更新', () => {
  it('確認してから更新し、完了を知らせる', async () => {
    const user = userEvent.setup();
    const { backend } = renderWithMock();
    await within(await fileList()).findByText('logo.png');
    act(() => useUiStore.getState().openDialog({ type: 'credentials' }));
    const dialog = await screen.findByRole('dialog');
    expect(await within(dialog).findByText(/この認証情報を使う接続/)).toBeInTheDocument();
    await user.type(within(dialog).getByLabelText('アクセスキー ID'), KEY_ID);
    await user.type(within(dialog).getByLabelText('シークレットアクセスキー'), SECRET);
    await user.click(within(dialog).getByRole('button', { name: '更新' }));
    await waitFor(() => expect(backend.calls.some((c) => c.cmd === 'credential_update')).toBe(true));
    expect(await findToast('認証情報を更新しました')).toHaveAttribute('data-tone', 'success');
  });
});

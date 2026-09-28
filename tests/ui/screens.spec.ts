// モックバックエンドでの画面操作（09 §2.4）。フォルダの移動、選択、コンテキストメニュー、ダイアログ、
// キーボードショートカット、ウィンドウ幅による切り替え（01 §5.5）を確かめる。

import { expect, openFiles, test } from './fixtures';

test('フォルダを開き、パンくずと戻るで移動する', async ({ app }) => {
  const list = await openFiles(app);
  await list.getByText('projects', { exact: true }).dblclick();
  await expect(list.getByText('brand-guidelines.pdf')).toBeVisible();
  const crumbs = app.getByRole('navigation', { name: 'パンくずリスト' });
  await expect(crumbs.getByRole('button', { name: 'projects' })).toHaveAttribute('aria-current', 'page');
  await app.getByRole('button', { name: '戻る' }).click();
  await expect(list.getByText('README.md')).toBeVisible();
});

test('クリック・修飾キーで選択し、ステータスバーに件数を出す', async ({ app }) => {
  const list = await openFiles(app);
  await list.getByText('README.md').click();
  await list.getByText('logo.png').click({ modifiers: ['ControlOrMeta'] });
  await expect(app.getByText(/2 項目を選択/)).toBeVisible();
  const inspector = app.getByRole('complementary', { name: 'インスペクタ' });
  await expect(inspector.getByText('2 項目', { exact: true })).toBeVisible();
});

test('右クリックでコンテキストメニューを開き、項目を選ぶ', async ({ app }) => {
  const list = await openFiles(app);
  await list.getByText('logo.png').click({ button: 'right' });
  const menu = app.getByRole('menu');
  for (const item of ['ダウンロード', '移動…', '名前を変更…', 'ストレージクラスを変更…', 'キーをコピー']) {
    await expect(menu.getByRole('menuitem', { name: new RegExp(item) })).toBeVisible();
  }
  await menu.getByRole('menuitem', { name: /名前を変更/ }).click();
  await expect(app.getByRole('dialog')).toContainText('名前を変更');
});

test('新規フォルダのダイアログで作成し、トーストで知らせる', async ({ app }) => {
  await openFiles(app);
  await app.getByRole('button', { name: '新規フォルダ' }).click();
  const dialog = app.getByRole('dialog');
  await dialog.getByLabel('名前').fill('reports');
  await dialog.getByRole('button', { name: '作成' }).click();
  await expect(app.getByText('「reports」を作成しました')).toBeVisible();
  await expect(app.getByRole('listbox', { name: 'acme-media-tokyo' }).getByText('reports')).toBeVisible();
});

test('キーボードショートカット（03 §10）', async ({ app }) => {
  const list = await openFiles(app);
  // ⌘A（すべてを選択）
  await list.getByText('README.md').click();
  await app.keyboard.press('ControlOrMeta+a');
  await expect(app.getByText(/6 項目を選択/)).toBeVisible();
  // Esc（選択の解除）、⇧⌘N（新規フォルダ）
  await app.keyboard.press('Escape');
  await expect(app.getByText(/項目を選択/)).toHaveCount(0);
  await app.keyboard.press('ControlOrMeta+Shift+N');
  await expect(app.getByRole('dialog')).toContainText('新規フォルダ');
  await app.keyboard.press('Escape');
  // ⌘F（検索にフォーカス）
  await app.keyboard.press('ControlOrMeta+f');
  await expect(app.getByRole('searchbox', { name: '検索' })).toBeFocused();
});

test.describe('ウィンドウ幅による切り替え（01 §5.5）', () => {
  test('1,100 px 以上: 5 列、インスペクタを常に表示', async ({ app }) => {
    await app.setViewportSize({ width: 1200, height: 760 });
    await openFiles(app);
    // 見出し行（名前・更新日・サイズ・種類・ストレージクラス）
    await expect(app.getByRole('main').getByText('種類', { exact: true })).toBeVisible();
    await expect(app.getByRole('complementary', { name: 'インスペクタ' })).toBeVisible();
  });

  test('900〜1,099 px: 「種類」列を省き、インスペクタは選択時だけ', async ({ app }) => {
    await app.setViewportSize({ width: 1000, height: 760 });
    const list = await openFiles(app);
    await expect(app.getByRole('main').getByText('種類', { exact: true })).toHaveCount(0);
    await expect(app.getByRole('complementary', { name: 'インスペクタ' })).toHaveCount(0);
    await list.getByText('README.md').click();
    await expect(app.getByRole('complementary', { name: 'インスペクタ' })).toBeVisible();
  });

  test('900 px 未満: インスペクタを重ねて表示する', async ({ app }) => {
    await app.setViewportSize({ width: 820, height: 760 });
    const list = await openFiles(app);
    await list.getByText('README.md').click();
    const inspector = app.getByRole('complementary', { name: 'インスペクタ' });
    await expect(inspector).toBeVisible();
    const box = await inspector.boundingBox();
    // 幅は 300 px または 85% の小さい方
    expect(box?.width).toBeLessThanOrEqual(300);
  });
});

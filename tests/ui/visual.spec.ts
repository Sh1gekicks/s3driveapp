// ビジュアル回帰テスト（09 §2.6）。主要な画面・ダイアログ・コンテキストメニューを、ライトとダークの両方で
// 基準画像と比べる。基準画像は CI と同じ Playwright の公式イメージ（Linux）で作る（docs/ope/03-test.md）。

import type { Page } from '@playwright/test';
import { expect, openFiles, test } from './fixtures';

for (const colorScheme of ['light', 'dark'] as const) {
  test.describe(`${colorScheme}`, () => {
    test.use({ colorScheme });

    const shot = (page: Page, name: string) =>
      expect(page).toHaveScreenshot(`${name}-${colorScheme}.png`, { fullPage: true });

    test('SCR-01 サインイン', async ({ app }) => {
      await app.goto('/?signedOut=1');
      await expect(app.getByRole('button', { name: 'Google でサインイン' })).toBeVisible();
      await shot(app, 'scr-01-signin');
    });

    test('SCR-01 バケットに接続', async ({ app }) => {
      await app.goto('/?empty=1');
      await expect(app.getByLabel('アクセスキー ID')).toBeVisible();
      await shot(app, 'scr-01-connect');
    });

    test('SCR-02 ファイルブラウザ', async ({ app }) => {
      const list = await openFiles(app);
      await list.getByText('README.md').click();
      await expect(
        app.getByRole('complementary', { name: 'インスペクタ' }).getByText('text/markdown'),
      ).toBeVisible();
      await shot(app, 'scr-02-browser');
    });

    test('SCR-03 ストレージとコスト', async ({ app }) => {
      await openFiles(app);
      await app.getByRole('navigation', { name: 'サイドバー' }).getByText('ストレージとコスト').click();
      await expect(app.getByText('ストレージクラス別')).toBeVisible();
      await shot(app, 'scr-03-dashboard');
    });

    test('コンテキストメニュー', async ({ app }) => {
      const list = await openFiles(app);
      await list.getByText('logo.png').click({ button: 'right' });
      await expect(app.getByRole('menu')).toBeVisible();
      await shot(app, 'mnu-01-context');
    });

    const dialogs: [string, (page: Page) => Promise<void>][] = [
      ['dlg-01-new-folder', (p) => p.getByRole('button', { name: '新規フォルダ' }).click()],
      [
        'dlg-02-delete',
        async (p) => {
          await p.getByRole('listbox', { name: 'acme-media-tokyo' }).getByText('logo.png').click();
          await p.keyboard.press('ControlOrMeta+Backspace');
        },
      ],
      [
        'dlg-04-storage-class',
        async (p) => {
          await p
            .getByRole('listbox', { name: 'acme-media-tokyo' })
            .getByText('logo.png')
            .click({ button: 'right' });
          await p.getByRole('menuitem', { name: /ストレージクラスを変更/ }).click();
        },
      ],
    ];
    for (const [name, open] of dialogs) {
      test(name, async ({ app }) => {
        await openFiles(app);
        await open(app);
        await expect(app.getByRole('dialog').or(app.getByRole('alertdialog'))).toBeVisible();
        await shot(app, name);
      });
    }
  });
}

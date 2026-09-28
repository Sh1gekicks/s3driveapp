// 画面テストの共通の準備（09 §2.4）。モックのデータは DS の基準日時（2026/09/27 14:40）に合わせてある。

import { test as base, expect, type Page } from '@playwright/test';

/** 画面の日時表示（「12 分前」など）を安定させるため、時計を基準日時に固定する。 */
export const NOW = new Date('2026-09-27T14:40:00+09:00');

export const test = base.extend<{ app: Page }>({
  app: async ({ page }, use) => {
    await page.clock.setFixedTime(NOW);
    await use(page);
  },
});

export { expect };

/** 一覧（listbox）を開いて待つ。 */
export async function openFiles(page: Page, query = '') {
  await page.goto(`/${query}`);
  const list = page.getByRole('listbox', { name: 'acme-media-tokyo' });
  await expect(list.getByText('README.md')).toBeVisible();
  return list;
}

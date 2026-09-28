// モックバックエンドでの画面テストとビジュアル回帰テスト（09 §2.4、§2.6）。
//
// `pnpm dev:mock` と同じモックで画面を動かし、WebKit で確かめる。スクリーンショットの比較は描画環境
// （フォント）に依存するため、CI と同じ Playwright の公式イメージ（Linux）で実行する（docs/ope/03-test.md）。

import { defineConfig, devices } from '@playwright/test';

const PORT = 4173;

export default defineConfig({
  testDir: 'tests/ui',
  // 基準画像は OS ごとに分けずに持つ（Linux のコンテナでだけ比較する）
  snapshotPathTemplate: '{testDir}/__screenshots__/{testFileName}/{arg}{ext}',
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL: `http://localhost:${PORT}`,
    locale: 'ja-JP',
    timezoneId: 'Asia/Tokyo',
    trace: 'retain-on-failure',
  },
  expect: {
    toHaveScreenshot: { animations: 'disabled', caret: 'hide', maxDiffPixelRatio: 0.005 },
  },
  projects: [
    {
      name: 'screens',
      testIgnore: /visual\.spec\.ts/,
      use: { ...devices['Desktop Safari'], viewport: { width: 1200, height: 760 } },
    },
    {
      name: 'visual',
      testMatch: /visual\.spec\.ts/,
      use: { ...devices['Desktop Safari'], viewport: { width: 1200, height: 760 } },
    },
  ],
  webServer: {
    // pnpm exec を挟むと終了のシグナルが Vite に届かず後始末で止まるため、node で直接起動する
    command: `node node_modules/vite/bin/vite.js --mode mock --port ${PORT} --strictPort`,
    url: `http://localhost:${PORT}`,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});

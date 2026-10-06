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
    // 同じイメージでの描画の揺れ（CI の amd64 と手元の arm64 の差を含む）は数階調のため、画素ごとの色の差の閾値
    // （threshold。既定の 0.2 は輝度で約 53 階調）を 0.05（約 13 階調）にし、超えた画素は 20 画素まで許す。
    // 既定の閾値と比率での許容では、一覧の列が 10px ずれても通ってしまう（09 §2.6）
    toHaveScreenshot: { animations: 'disabled', caret: 'hide', threshold: 0.05, maxDiffPixels: 20 },
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

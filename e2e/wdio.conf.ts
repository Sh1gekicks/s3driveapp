// E2E テスト（09 §2.5）。`e2e` 機能を有効にしたアプリを、アプリに内蔵した WebDriver（embedded）で操作する。
//
// 前提:
//   - moto が S3DRIVE_TEST_ENDPOINT（既定 http://localhost:5000）で動いていること
//   - `pnpm tauri build --debug --no-bundle --features e2e` でアプリをビルドしてあること
// 実行: pnpm e2e

import { mkdirSync, mkdtempSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { browser } from '@wdio/globals';

const root = resolve(import.meta.dirname, '..');
const binary = process.env.S3DRIVE_E2E_BINARY ?? join(root, 'target/debug/s3drive-app');

export interface Fixture {
  name: string;
  body: string;
  /** ダウンロード時に復元されることを確かめる更新日時（ISO 8601） */
  mtime: string;
  downloadDir: string;
}

// アップロードするファイルと保存先。ファイル選択ダイアログの代わりに、アプリは環境変数のパスで選択 ID を発行する。
// 設定ファイルはランナーとワーカーの両方で読み込まれるため、最初に作ったものを環境変数で引き継ぐ
if (!process.env.S3DRIVE_E2E_FIXTURE) {
  const work = mkdtempSync(join(tmpdir(), 's3drive-e2e-'));
  const uploadDir = join(work, 'upload');
  const downloadDir = join(work, 'download');
  mkdirSync(uploadDir);
  mkdirSync(downloadDir);
  const fixture: Fixture = {
    name: `hello-${Date.now()}.txt`,
    body: 'S3 Drive E2E\n',
    mtime: '2026-01-02T03:04:05.000Z',
    downloadDir,
  };
  const uploadPath = join(uploadDir, fixture.name);
  writeFileSync(uploadPath, fixture.body);
  utimesSync(uploadPath, new Date(fixture.mtime), new Date(fixture.mtime));
  process.env.S3DRIVE_E2E_UPLOAD_PATHS = uploadPath;
  process.env.S3DRIVE_E2E_DOWNLOAD_DIR = downloadDir;
  process.env.S3DRIVE_E2E_FIXTURE = JSON.stringify(fixture);
}
process.env.S3DRIVE_TEST_ENDPOINT ??= 'http://localhost:5000';

export const config: WebdriverIO.Config = {
  runner: 'local',
  specs: ['./specs/**/*.e2e.ts'],
  // シナリオは前のシナリオの結果（接続・アップロードしたファイル）を使うため、順に 1 つのセッションで実行する
  maxInstances: 1,
  capabilities: [
    {
      browserName: 'tauri',
      'tauri:options': { application: binary },
    } as WebdriverIO.Capabilities,
  ],
  services: [['@wdio/tauri-service', { appBinaryPath: binary, driverProvider: 'embedded' }]],
  logLevel: 'warn',
  bail: 1,
  waitforTimeout: 15_000,
  connectionRetryTimeout: 120_000,
  connectionRetryCount: 2,
  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: { ui: 'bdd', timeout: 120_000, bail: true },
  // メインウィンドウを明示的に選ぶ。@wdio/tauri-service はコマンドごとにウィンドウの状態を tauri-plugin-wdio に
  // 問い合わせてフォーカスを合わせるが、このアプリはそのプラグインを組み込んでいないため（09 §2.5）、明示的な
  // switchToWindow でこの処理を止める（止めないと 1 コマンドごとに 5 秒待つ）
  before: async () => {
    await browser.switchToWindow('main');
  },
  // 失敗したシナリオの画面を保存する（CI では成果物として残す）
  afterTest: async (test, _context, result) => {
    if (result.passed) return;
    const dir = process.env.S3DRIVE_E2E_ARTIFACTS ?? join(root, 'e2e/.artifacts');
    mkdirSync(dir, { recursive: true });
    await browser.saveScreenshot(join(dir, `${test.title.replace(/[\\/:*?"<>|\s]+/g, '_')}.png`));
  },
};

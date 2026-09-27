// 09 §2.5 のシナリオ。前のシナリオの結果（接続・アップロードしたファイル）を使うため、1 つのファイルに順に書く。

import { existsSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { $, browser, expect } from '@wdio/globals';
import {
  BUCKET,
  clickButton,
  contextMenu,
  dialog,
  expectToast,
  field,
  fileList,
  fixture,
  menuAction,
  openRow,
  row,
  selectRow,
  textXPath,
} from '../helpers';

const f = fixture();
const FOLDER = 'e2e-folder';

describe('S3 Drive', () => {
  it('サインイン（テスト用）→ バケットに接続 → 一覧を表示する', async () => {
    const signIn = await $('button=Google でサインイン');
    if (await signIn.isExisting()) await signIn.click();

    await (await field('アクセスキー ID')).setValue('AKIAIOSFODNN7EXAMPLE');
    await (await field('シークレットアクセスキー')).setValue('wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY');
    await (await field('リージョン')).selectByAttribute('value', 'us-east-1');
    await (await field('バケット名')).setValue(BUCKET);
    await clickButton('接続');

    // 空のバケットなので一覧の代わりに空の状態を表示する
    await expect($(textXPath('ファイルをドロップしてアップロード'))).toBeDisplayed({ wait: 30_000 });
    await expect($('nav[aria-label="サイドバー"]').$(`span=${BUCKET}`)).toBeDisplayed();
  });

  it('ファイルをアップロードすると一覧に表示され、インスペクタにメタデータが出る', async () => {
    await clickButton('アップロード');
    await expectToast('アップロードが完了しました');
    await expect(row(f.name)).toBeDisplayed({ wait: 15_000 });

    await selectRow(f.name);
    const inspector = await $('aside[aria-label="インスペクタ"]');
    await expect(inspector.$(textXPath('text/plain'))).toBeDisplayed({ wait: 15_000 });
    await expect(inspector.$(`dd=${f.name}`)).toBeDisplayed();
  });

  it('ダウンロードすると保存先にファイルができ、更新日時が復元される', async () => {
    await selectRow(f.name);
    await menuAction('file.download_to');
    await expectToast('ダウンロードが完了しました');

    const saved = join(f.downloadDir, f.name);
    await browser.waitUntil(() => existsSync(saved), {
      timeout: 30_000,
      timeoutMsg: `${saved} がありません`,
    });
    expect(readFileSync(saved, 'utf8')).toBe(f.body);
    expect(Math.round(statSync(saved).mtimeMs / 1000)).toBe(Math.round(Date.parse(f.mtime) / 1000));
  });

  it('削除 → 削除済みの項目を表示 → 復元する', async () => {
    await contextMenu(f.name, '削除');
    const confirm = await dialog();
    await confirm.$('button=削除').click();
    await expectToast('1 項目を削除しました');
    await expect(row(f.name)).not.toBeDisplayed({ wait: 15_000 });

    await menuAction('view.deleted');
    await expect(row(f.name)).toBeDisplayed({ wait: 15_000 });
    await contextMenu(f.name, '復元');
    await expectToast('1 項目を復元しました');

    await menuAction('view.deleted');
    await expect(row(f.name)).toBeDisplayed({ wait: 15_000 });
  });

  it('新規フォルダを作成し、ファイルをそのフォルダへ移動する', async () => {
    await clickButton('新規フォルダ');
    const create = await dialog();
    const input = await create.$('aria/名前');
    await input.clearValue();
    await input.setValue(FOLDER);
    await create.$('button=作成').click();
    await expectToast(`「${FOLDER}」を作成しました`);
    await expect(row(FOLDER)).toBeDisplayed();

    await contextMenu(f.name, '移動…');
    const moveDialog = await dialog();
    await moveDialog.$(textXPath(FOLDER, 'treeitem')).click();
    await moveDialog.$('button=移動').click();
    await expectToast('1 項目を移動しました');
    await expect(row(f.name)).not.toBeDisplayed({ wait: 15_000 });

    await openRow(FOLDER);
    await expect(row(f.name)).toBeDisplayed({ wait: 15_000 });
  });

  it('ストレージクラスを変更すると一覧のバッジが変わる', async () => {
    await contextMenu(f.name, 'ストレージクラスを変更…');
    const change = await dialog();
    await change.$(textXPath('Standard-IA', 'radio')).click();
    await change.$('button=変更').click();
    await expectToast('ストレージクラスを変更しました');
    await expect(fileList().$(textXPath('Standard-IA'))).toBeDisplayed({ wait: 15_000 });
  });

  it('拡張子で検索すると結果が表示される', async () => {
    await $('aria/フィルタ').click();
    await $('aria/拡張子').setValue('txt');
    await expect($(textXPath(`${BUCKET} 全体を検索中`))).toBeDisplayed({ wait: 30_000 });
    await expect(row(f.name)).toBeDisplayed({ wait: 30_000 });
    await expect(fileList().$(`span=${FOLDER}/`)).toBeDisplayed();
  });
});

// E2E テストの補助。要素はアクセシブルな名前（aria/…）と表示文字列で探す。

import { $, browser, expect } from '@wdio/globals';
import type { ChainablePromiseElement } from 'webdriverio';
import type { Fixture } from './wdio.conf';

export const BUCKET = process.env.S3DRIVE_E2E_BUCKET ?? 's3drive-e2e';

export function fixture(): Fixture {
  const raw = process.env.S3DRIVE_E2E_FIXTURE;
  if (!raw) throw new Error('S3DRIVE_E2E_FIXTURE がありません（wdio.conf.ts を経由して実行してください）');
  return JSON.parse(raw) as Fixture;
}

/** 文字列を含む要素の XPath（WebdriverIO の `*=` はタグを省くとリンクだけを探すため使わない）。 */
export function textXPath(text: string, role?: string): string {
  const literal = JSON.stringify(text);
  return role
    ? `.//*[@role="${role}"][contains(normalize-space(.), ${literal})]`
    : `.//*[text()[contains(., ${literal})]]`;
}

/** ファイル一覧（listbox）。 */
export function fileList(): ChainablePromiseElement {
  return $(`[role="listbox"][aria-label="${BUCKET}"]`);
}

/** 一覧の行（一覧が空で listbox がない場合も「見つからない」として扱えるよう、1 つの XPath で探す）。 */
export function row(name: string): ChainablePromiseElement {
  return $(
    `//*[@role="listbox"][@aria-label=${JSON.stringify(BUCKET)}]//span[normalize-space(.)=${JSON.stringify(name)}]`,
  );
}

/** ラベルの文字列から入力欄を探す（aria/ セレクタは select 要素の名前を求められないため、label の for を辿る）。 */
export async function field(label: string): Promise<ChainablePromiseElement> {
  const id = await $(`label=${label}`).getAttribute('for');
  return $(`[id="${id}"]`);
}

export async function clickButton(name: string) {
  // 文字のボタンとアイコンだけのボタン（aria-label）の両方を探す
  const literal = JSON.stringify(name);
  const button = await $(`//button[normalize-space(.)=${literal} or @aria-label=${literal}]`);
  await button.waitForClickable();
  await button.click();
}

/**
 * マウス操作のイベントを要素の中央で発生させる。アプリに内蔵した WebDriver のクリックは `element.click()` だけを
 * 呼ぶため、選択（mousedown）・コンテキストメニュー（contextmenu）・ダブルクリック（dblclick）は直接発生させる。
 */
async function mouse(element: ChainablePromiseElement | WebdriverIO.Element, types: string[], button = 0) {
  await browser.execute(
    (el: HTMLElement, names: string[], b: number) => {
      el.scrollIntoView({ block: 'center' });
      const rect = el.getBoundingClientRect();
      const init = {
        bubbles: true,
        cancelable: true,
        view: window,
        button: b,
        buttons: b === 2 ? 2 : 1,
        clientX: rect.left + rect.width / 2,
        clientY: rect.top + rect.height / 2,
      };
      for (const name of names) el.dispatchEvent(new MouseEvent(name, init));
    },
    element as unknown as HTMLElement,
    types,
    button,
  );
}

/** 行をクリックして選択する。 */
export async function selectRow(name: string) {
  const target = await row(name);
  await target.waitForDisplayed();
  await mouse(target, ['mousedown', 'mouseup', 'click']);
}

/** 行をダブルクリックして開く。 */
export async function openRow(name: string) {
  const target = await row(name);
  await target.waitForDisplayed();
  await mouse(target, ['mousedown', 'mouseup', 'click', 'mousedown', 'mouseup', 'click', 'dblclick']);
}

/** 行を右クリックし、コンテキストメニューの項目を選ぶ。 */
export async function contextMenu(name: string, item: string) {
  const target = await row(name);
  await target.waitForDisplayed();
  await mouse(target, ['mousedown', 'mouseup', 'contextmenu'], 2);
  const menuItem = await $(textXPath(item, 'menuitem'));
  await menuItem.waitForDisplayed();
  await menuItem.click();
}

/** 表示中のダイアログ（dialog または alertdialog）。 */
export async function dialog(): Promise<ChainablePromiseElement> {
  const el = $('[role="dialog"], [role="alertdialog"]');
  await el.waitForDisplayed();
  return el;
}

/** トーストの表示を待つ。 */
export async function expectToast(text: string) {
  await expect($(textXPath(text))).toBeDisplayed({ wait: 30_000 });
}

/**
 * メニューバーの項目を選んだときと同じイベントを送る（ネイティブのメニューは WebDriver で操作できないため）。
 * フロントエンドは `menu://action` を購読している（05 §4.2）。
 */
export async function menuAction(id: string) {
  await browser.execute(async (menuId) => {
    const internals = (
      window as unknown as {
        __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> };
      }
    ).__TAURI_INTERNALS__;
    await internals.invoke('plugin:event|emit', { event: 'menu://action', payload: { id: menuId } });
  }, id);
}

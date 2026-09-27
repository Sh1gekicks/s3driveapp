# デザインシステムのスナップショット

このフォルダは、Claude Design のプロジェクト「S3 Drive デザインシステム」の写しである。設計書（`docs/design/`）で `DS:` を付けて参照しているファイルは、このフォルダを基準とする（例: `DS: tokens/colors.css` → `design-system/tokens/colors.css`）。

| 項目 | 内容 |
|---|---|
| 取得元 | https://claude.ai/design/p/2d5c9b6d-736c-4a1d-8ca7-8d2787602a5e |
| 取得日 | 2026-09-27 |
| ファイル数 | 108（プロジェクトのサムネイル `.thumbnail` は除く） |

## 取得時の処理

Claude Design からファイルを取得すると、表示用の内容が付加される。次のものを取り除き、Claude Design 上のファイル一覧に表示されるサイズと全ファイルが一致することを確認した。

- HTML: `<head>` 直後に付加されるプレビュー用の `<style>`・`<script>`（`data-omelette-injected` 属性付き）
- SVG・JPEG: 付加される来歴情報（C2PA のマニフェスト）

## 扱い方

- 正本は Claude Design のプロジェクトである。このフォルダを直接編集せず、Claude Design で変更してから取り込み直す。
- 取り込み直すときは、差分を確認してから、トークンなどの取り込み先（`docs/design/02-ui-foundation.md` の §2）にも反映する。
- `components/**/*.jsx` と `ui_kits/` はモックで、アプリのコードではない。Biome・TypeScript・テストの対象から除外する。
- `ui_kits/s3-drive/index.html` などの見本は、ブラウザで開くと React・Babel・Lucide を CDN から読み込んで表示する。

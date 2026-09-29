# 02. UI 基盤（デザインシステムの実装）

## 1. デザインシステムの概要

| 項目 | 内容 |
|---|---|
| 名称 | S3 Drive デザインシステム（Claude Design） |
| 構成 | トークン（`DS: tokens/`）、コンポーネント 19 種（`DS: components/`）、UI キット（`DS: ui_kits/s3-drive/`）、ファウンデーション見本（`DS: guidelines/`）、アイコン素材（`DS: assets/`） |
| 基本方針 | 「Finder の隣に置いて違和感のないアプリ」。装飾ではなく macOS のマテリアル・密度・操作感に合わせる。ブランド色は Azure の 1 色だけで、選択・主要アクション・フォーカスに使う |
| 色 | クールグレー（hue 250）＋ Azure アクセント（hue 240）。セマンティックトークンは shadcn/ui 互換の名前（`--background`、`--primary` など） |
| 文字 | `-apple-system`（SF Pro）と Hiragino Sans。本文 13px（macOS HIG） |
| 密度 | 4px グリッド。コントロール高 24／28／32px、リスト行 28px |

実装では DS のトークン CSS の値を正とする。DS の JSX（`DS: components/**/*.jsx`、`DS: ui_kits/**`）はモックなので、コードは流用せず仕様として参照し、shadcn/ui（Base UI 版）のコンポーネントを DS の見た目に合わせてカスタマイズする。

## 2. 取り込みと同期の方針

| DS のファイル | 取り込み先 | 扱い |
|---|---|---|
| `tokens/colors.css` | `src/styles/tokens/colors.css` | そのままコピーする（ライト／ダークの切り替えを含む） |
| `tokens/typography.css`、`tokens/spacing.css` | `src/styles/globals.css` の `@theme`、`src/styles/tokens/layout.css` | Tailwind のテーマ変数に移植する（§4） |
| `tokens/base.css` | `src/styles/globals.css` の `@layer base` | そのまま移植する |
| `components/components.css` | 各コンポーネントの className | 数値（高さ・余白・角丸・影・色）を参照して Tailwind のクラスに置き換える |
| `assets/*.svg` | `src-tauri/icons/`、`src/assets/` | アプリアイコンとメニューバーアイコンの素材（§8） |

取り込み元は、リポジトリの `design-system/`（Claude Design からの写し。[SNAPSHOT.md](../../design-system/SNAPSHOT.md)）とする。DS を更新したときは、まず `design-system/` を取り込み直し、差分を確認してから上表の取り込み先へ反映し、ビジュアル回帰テスト（[09 §2.6](09-testing.md#26-ビジュアル回帰テスト)）で確認する。値を変えたい場合は DS 側を先に更新する。

## 3. デザイントークン

### 3.1 セマンティックカラー

主要なトークンを示す（値は `DS: tokens/colors.css`）。

| トークン | 用途 | ライト | ダーク |
|---|---|---|---|
| `--background` | ウィンドウ・コンテンツ面 | `oklch(0.995 0.001 250)` | `oklch(0.2 0.006 250)` |
| `--foreground` | 本文 | `--gray-900` | `oklch(0.96 0.003 250)` |
| `--card` | カード（ダッシュボード、ダイアログ） | `oklch(1 0 0)` | `oklch(0.235 0.007 250)` |
| `--popover` | メニュー・トースト（半透明） | `oklch(0.99 0.002 250 / 0.9)` | `oklch(0.26 0.008 250 / 0.88)` |
| `--primary` | 主要アクション・選択・フォーカス | `--azure-600` | `oklch(0.56 0.16 240)` |
| `--muted` / `--muted-foreground` | 補助面 / 補助テキスト | `--gray-100` / `--gray-500` | `oklch(0.27 …)` / `oklch(0.72 …)` |
| `--accent` | ホバー面 | `oklch(0 0 0 / 0.05)` | `oklch(1 0 0 / 0.07)` |
| `--destructive` / `--success` / `--warning` / `--info` | 状態色 | `oklch(0.577 0.215 27)` ほか | `oklch(0.66 0.19 25)` ほか |
| `--border` / `--input` / `--ring` | 境界線 / 入力枠 / フォーカスリング | `--gray-200` / `--gray-300` / `--azure-500` | 白の半透明 / 白の半透明 / `--azure-400` |
| `--field-bg` | 入力欄の背景 | `oklch(1 0 0)` | `oklch(1 0 0 / 0.05)` |
| `--sidebar` / `--sidebar-foreground` | サイドバーの色味（vibrancy に重ねる） | `oklch(0.955 0.004 250 / 0.78)` | `oklch(0.23 0.007 250 / 0.72)` |
| `--sidebar-accent` / `--sidebar-border` | サイドバーの選択面 / 境界 | 黒 5% / 黒 9% | 白 6% / 黒 50% |
| `--row-hover` / `--row-stripe` | 行のホバー / 縞 | 黒 3.5% / 黒 1.8% | 白 4% / 白 2.5% |
| `--row-selected` / `--row-selected-foreground` | 選択行 | `--azure-600` / 白 | `oklch(0.52 0.15 240)` / 白 |
| `--row-selected-inactive` | ウィンドウ非アクティブ時の選択行 | 黒 8% | 白 10% |
| `--overlay` | ダイアログ背面 | `oklch(0.2 0.01 250 / 0.22)` | `oklch(0 0 0 / 0.45)` |
| `--tooltip-bg` / `--tooltip-foreground` | ツールチップ | `--gray-900` / `--gray-50` | `oklch(0.32 …)` / `oklch(0.96 …)` |
| `--shadow-xs`〜`--shadow-lg` | 影（0.5px の輪郭リング＋柔らかいドロップ。ダークは上端に 0.5px のハイライト） | §3.4 | §3.4 |

ステータス色（success／warning／destructive）の文字色は `color-mix` で foreground 側に寄せてコントラストを確保する（DS の方針）。

### 3.2 識別色（ストレージクラス・ファイル種別）

明度・彩度をそろえて色相だけを変えた識別色。凡例やバッジのドットに使う。

| ストレージクラス | トークン | 値 |
|---|---|---|
| STANDARD | `--sc-standard` | `oklch(0.64 0.14 240)` |
| INTELLIGENT_TIERING | `--sc-intelligent-tiering` | `oklch(0.64 0.14 195)` |
| STANDARD_IA | `--sc-standard-ia` | `oklch(0.64 0.14 155)` |
| ONEZONE_IA | `--sc-onezone-ia` | `oklch(0.64 0.14 115)` |
| GLACIER_IR | `--sc-glacier-ir` | `oklch(0.64 0.14 285)` |
| GLACIER | `--sc-glacier` | `oklch(0.64 0.14 310)` |
| DEEP_ARCHIVE | `--sc-deep-archive` | `oklch(0.64 0.14 340)` |

| ファイル種別 | トークン | 値 |
|---|---|---|
| フォルダ | `--ft-folder` | `oklch(0.66 0.14 240)` |
| 画像 | `--ft-image` | `oklch(0.66 0.14 340)` |
| ムービー | `--ft-video` | `oklch(0.66 0.14 290)` |
| オーディオ | `--ft-audio` | `oklch(0.66 0.14 20)` |
| PDF | `--ft-pdf` | `oklch(0.62 0.17 27)` |
| スプレッドシート | `--ft-sheet` | `oklch(0.66 0.14 155)` |
| アーカイブ | `--ft-archive` | `oklch(0.7 0.13 75)` |
| ソースコード | `--ft-code` | `oklch(0.66 0.14 195)` |
| 書類（その他） | `--ft-doc` | `oklch(0.6 0.02 250)` |

### 3.3 タイポグラフィ

| トークン | 値 | 用途 |
|---|---|---|
| `--font-sans` | `-apple-system, BlinkMacSystemFont, "SF Pro Text", "Hiragino Sans", "Hiragino Kaku Gothic ProN", "Helvetica Neue", sans-serif` | 全体（REQ-D03） |
| `--font-display` | `-apple-system, BlinkMacSystemFont, "SF Pro Display", "Hiragino Sans", …` | 大きな数値・見出し |
| `--font-mono` | `ui-monospace, "SF Mono", SFMono-Regular, Menlo, monospace` | キー、ETag、ARN、バージョン ID（11〜12px） |
| `--text-2xs` | 10px | キャプション |
| `--text-xs` | 11px | セクション見出し、バッジ、ステータスバー |
| `--text-sm` | 12px | 補助ラベル、表のセル |
| `--text-base` | 13px | 本文・全コントロールの既定 |
| `--text-md` | 15px | ダイアログのタイトル |
| `--text-lg` | 17px | title2 |
| `--text-xl` | 22px | title1、KPI の数値 |
| `--text-2xl` | 26px | 大きな数値 |

数値を表示する箇所は `font-variant-numeric: tabular-nums` を指定する。

### 3.4 余白・サイズ・角丸・影・モーション

| 区分 | トークン | 値 |
|---|---|---|
| 余白 | `--space-0-5`〜`--space-10` | 2, 4, 6, 8, 12, 16, 20, 24, 32, 40px（4px グリッド + 2/6px） |
| コントロール高 | `--control-h-sm` / `-md` / `-lg` | 24 / 28 / 32px |
| 行 | `--row-h` | 28px |
| レイアウト | `--titlebar-h` / `--sidebar-w` / `--inspector-w` | 52 / 220 / 280px（`--sidebar-w` は既定値。幅を変えるとサイドバーの要素で上書きする） |
| ヘアライン | `--hairline` | 0.5px |
| 角丸 | `--radius-xs`〜`--radius-2xl`、`--radius-window` | 4（チェック）/ 5（バッジ）/ 6（ボタン・入力・行）/ 8（メニュー）/ 10（トースト・カード）/ 12（ダイアログ・ウィンドウ） |
| 影 | `--shadow-xs` / `-sm` / `-md` / `-lg` | 入力欄 / カード・セグメント / メニュー・トースト / ダイアログ・ウィンドウ |
| モーション | `--dur-fast` / `--dur-base` / `--dur-slow` | 120ms（ホバー・色）/ 200ms（スイッチ）/ 320ms（プログレス） |
| イージング | `--ease-out` | `cubic-bezier(0.2, 0, 0, 1)`。バウンスは使わない |
| マテリアル | `--material-blur` | `saturate(180%) blur(24px)`（メニュー・トーストのみ） |

## 4. Tailwind CSS v4 への割り当て

`src/styles/globals.css` の構成を示す。

```css
@import "tailwindcss";
@import "./tokens/colors.css";   /* DS: tokens/colors.css（ライト／ダーク） */
@import "./tokens/layout.css";   /* DS: tokens/spacing.css のうちサイズ・モーション・マテリアル */

/* dark: バリアントを OS 設定（自動）と明示指定の両方に追従させる */
@custom-variant dark {
  @media (prefers-color-scheme: dark) {
    &:where(:root:not([data-theme="light"]) *) { @slot; }
  }
  &:where([data-theme="dark"] *) { @slot; }
}

@theme {
  /* DS: tokens/typography.css */
  --font-sans: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Hiragino Sans",
    "Hiragino Kaku Gothic ProN", "Helvetica Neue", sans-serif;
  --font-display: -apple-system, BlinkMacSystemFont, "SF Pro Display", "Hiragino Sans",
    "Helvetica Neue", sans-serif;
  --font-mono: ui-monospace, "SF Mono", SFMono-Regular, Menlo, monospace;
  --text-2xs: 10px;
  --text-xs: 11px;
  --text-sm: 12px;
  --text-base: 13px;
  --text-md: 15px;
  --text-lg: 17px;
  --text-xl: 22px;
  --text-2xl: 26px;

  /* DS: tokens/spacing.css（4px グリッド） */
  --spacing: 4px;
  --radius-xs: 4px;
  --radius-sm: 5px;
  --radius-md: 6px;
  --radius-lg: 8px;
  --radius-xl: 10px;
  --radius-2xl: 12px;
  --ease-out: cubic-bezier(0.2, 0, 0, 1);
  --ease-in-out: cubic-bezier(0.4, 0, 0.2, 1);
}

:root {
  --radius: 6px; /* shadcn/ui の基準角丸（DS: 「--radius は 6px を基準に」） */
}

@theme inline {
  --color-background: var(--background);
  --color-foreground: var(--foreground);
  --color-card: var(--card);
  --color-card-foreground: var(--card-foreground);
  --color-popover: var(--popover);
  --color-popover-foreground: var(--popover-foreground);
  --color-primary: var(--primary);
  --color-primary-foreground: var(--primary-foreground);
  --color-secondary: var(--secondary);
  --color-secondary-foreground: var(--secondary-foreground);
  --color-muted: var(--muted);
  --color-muted-foreground: var(--muted-foreground);
  --color-accent: var(--accent);
  --color-accent-foreground: var(--accent-foreground);
  --color-destructive: var(--destructive);
  --color-success: var(--success);
  --color-warning: var(--warning);
  --color-info: var(--info);
  --color-border: var(--border);
  --color-input: var(--input);
  --color-ring: var(--ring);
  --color-field: var(--field-bg);
  --color-sidebar: var(--sidebar);
  --color-sidebar-foreground: var(--sidebar-foreground);
  --color-sidebar-accent: var(--sidebar-accent);
  --color-sidebar-border: var(--sidebar-border);
  --color-row-hover: var(--row-hover);
  --color-row-stripe: var(--row-stripe);
  --color-row-selected: var(--row-selected);
  --color-row-selected-foreground: var(--row-selected-foreground);
  --color-row-selected-inactive: var(--row-selected-inactive);
  --color-overlay: var(--overlay);
  --color-sc-standard: var(--sc-standard);
  --color-sc-intelligent-tiering: var(--sc-intelligent-tiering);
  --color-sc-standard-ia: var(--sc-standard-ia);
  --color-sc-onezone-ia: var(--sc-onezone-ia);
  --color-sc-glacier-ir: var(--sc-glacier-ir);
  --color-sc-glacier: var(--sc-glacier);
  --color-sc-deep-archive: var(--sc-deep-archive);
  /* --ft-* も同様に --color-ft-* として割り当てる */
}

/* 影はライト／ダークで値が変わるため、変数を直接参照するユーティリティにする */
@utility elevation-xs { box-shadow: var(--shadow-xs); }
@utility elevation-sm { box-shadow: var(--shadow-sm); }
@utility elevation-md { box-shadow: var(--shadow-md); }
@utility elevation-lg { box-shadow: var(--shadow-lg); }
@utility material {
  background: var(--popover);
  -webkit-backdrop-filter: var(--material-blur);
  backdrop-filter: var(--material-blur);
}
@utility selectable { -webkit-user-select: text; user-select: text; cursor: text; }

@layer base {
  /* DS: tokens/base.css の内容をここに置く（§7.3） */
}
```

注意点:

- DS の `base.css` は `html` にも `font-size: 13px` を指定するため、rem 基準の Tailwind 既定値（余白・文字サイズ）がずれる。`--spacing` と `--text-*` を px で上書きして回避する。
- Tailwind の影ユーティリティ（`shadow-sm` など）はテーマの値を埋め込むため、ライト／ダークで変わる DS の影には使わない。`elevation-*` を使う。
- レイアウト寸法は `h-(--row-h)`、`w-(--sidebar-w)` のように変数を直接参照する。

## 5. shadcn/ui on Base UI のセットアップ

1. Tauri の雛形（React + TypeScript + Vite）を pnpm 12 で作成し、`package.json`・`pnpm-workspace.yaml`・tsconfig を [01 §8](01-architecture.md#8-開発環境とツールチェーン) の内容にする（Node.js 26、TypeScript 7）。
2. Tailwind CSS v4 を導入する: `pnpm add tailwindcss @tailwindcss/vite`、`vite.config.ts` に `tailwindcss()` とパスエイリアス `@` を追加する。tsconfig のエイリアスは `paths` だけで設定する。shadcn/ui の Vite 向けの手順は `baseUrl` の追加を案内しているが、TypeScript 7 では `baseUrl` がエラーになるため追加しない。
3. shadcn/ui を Base UI 版で初期化する。Base UI は 2026 年 7 月以降の既定値だが、`--base base` で明示する。`--no-pointer` でボタンのカーソルを `default` のままにする（REQ-D05）。初期化後、CLI が `paths` のエイリアス（`@/components` など）を正しく解決していることを `components.json` で確認する。

   ```sh
   pnpm dlx shadcn@latest init --base base --no-pointer
   ```

4. 使うコンポーネントを追加する。

   ```sh
   pnpm dlx shadcn@latest add button input input-group field native-select checkbox switch \
     toggle-group badge progress toast tooltip dialog alert-dialog dropdown-menu context-menu \
     breadcrumb separator kbd spinner empty skeleton
   ```

5. 生成されたテーマ CSS を §4 の構成に置き換え、各コンポーネントのサイズ・角丸・色を §6 に合わせて調整する。

`components.json` の例（`style` は `base-<プリセット名>`。見た目は DS のトークンで上書きするため、プリセットは初期化時の既定でよい）:

```json
{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "base-nova",
  "rsc": false,
  "tsx": true,
  "tailwind": {
    "config": "",
    "css": "src/styles/globals.css",
    "baseColor": "neutral",
    "cssVariables": true
  },
  "iconLibrary": "lucide",
  "aliases": {
    "components": "@/components",
    "ui": "@/components/ui",
    "lib": "@/lib",
    "utils": "@/lib/utils",
    "hooks": "@/hooks"
  }
}
```

## 6. コンポーネント対応表

| DS コンポーネント | 実装 | DS 仕様の要点 | 実装上の調整 |
|---|---|---|---|
| Button | `ui/button` | variant: default／secondary／outline／ghost／destructive／link。size: sm 24／md 28／lg 32px。13px・medium・角丸 6。1 画面に default は 1 つ | size 名を DS に合わせる。ホバーは背景を 6〜10% 濃く、押下はさらに濃く。拡大縮小しない |
| IconButton | `ui/button`（icon サイズ）＋ `ui/tooltip` | `label` 必須（アクセシブル名とツールチップ）。`active` で押下状態 | `aria-label` と `aria-pressed` を付ける。ツールチップにショートカットを併記する |
| Icon | `lucide-react` | 線幅 1.75。ツールバー・行 16px、小ボタン 14px、バッジ 12px、空状態 32px | 既定値を持つ `<Icon>` ラッパーを用意する |
| Input | `ui/input`、`ui/field`、`ui/input-group` | ラベルは上、ヒント・エラーは下。高さ 28（sm 24）。先頭アイコン（検索）。入力文字は選択可 | 枠はヘアライン、背景 `--field-bg`、影 `--shadow-xs`。フォーカス時は枠も `--ring` |
| Select | `ui/native-select` | macOS のポップアップボタン（ネイティブの select ＋上下シェブロン） | WKWebView ではネイティブのメニューが開き macOS らしいため、Base UI の Select ではなく native-select を使う |
| Checkbox | `ui/checkbox` | 14px。未確定状態あり | |
| Switch | `ui/switch` | macOS 風トグル。設定行ではラベル左・スイッチ右 | |
| SegmentedControl | `ui/toggle-group`（単一選択） | 表示切り替え、インスペクタのタブ。`block` で等幅 | 選択中のセグメントは `--seg-active` と `elevation-sm` |
| Badge | `ui/badge` | 高さ 18px。default／secondary／outline／success／warning／destructive。アイコン付き | success／warning の variant を追加する |
| StorageClassBadge | `ds/storage-class-badge` | ドット（`--sc-*`）＋名前。`short`（短い名前）、`plain`（表のセル用、背景なし） | DS 独自 |
| Progress | `ui/progress`、`ds/usage-bar` | 転送の進捗（高さ 6）と、クラス別の積み上げ（セグメント） | 積み上げは DS 独自の `UsageBar` にする |
| Toast | `ui/toast`（Base UI の Toast） | 右下に積む。幅 340、マテリアル、アイコン・トーン・進捗バー・アクション | shadcn の Base UI 版は Sonner ではなく toast を使う |
| Tooltip | `ui/tooltip` | 暗い小さなラベル。ショートカットがあれば併記 | ショートカットは `ui/kbd` で表記 |
| Dialog | `ui/alert-dialog`（確認）、`ui/dialog`（入力） | ウィンドウ内のオーバーレイ。色付きタイルのアイコン、tone（default／destructive）、主ボタンは右端。幅 420 が既定 | 背面は `--overlay`。角丸 12、影 `--shadow-lg` |
| Menu | `ui/context-menu`、`ui/dropdown-menu` | 幅 220。マテリアル。ホバーで primary 塗り＋白文字（macOS 準拠）。ショートカット表示、破壊的項目 | ハイライト色を DS に合わせる |
| SidebarItem / SidebarSection | `ds/sidebar-item`、`ds/sidebar-section` | 行 28px、見出し 11px semibold・muted | shadcn の `sidebar` はレイアウト機構が大きく、vibrancy やタイトルバー領域の扱いが DS と異なるため使わない |
| Breadcrumbs | `ui/breadcrumb` | 先頭はバケット（database アイコン）、最後が現在のフォルダ | 幅が足りないときは中間を省略記号にまとめる |
| FileIcon | `ds/file-icon` | 拡張子から種別を判定し、Lucide のグリフ＋`--ft-*` 色＋ 22% の塗り | 対応表は §8.2 |
| AppWindow | `ds/app-shell` | サイドバー 220、ツールバー 52（タイトルバーを兼ねる）、インスペクタ 280 | 信号機ボタンは OS が描画するため、DS の `TrafficLights` モックは使わない |

読み込み中の表示には `ui/skeleton`（一覧）と `ui/spinner`（ボタン内）、空状態には `ui/empty` を使う。

## 7. macOS ネイティブ化の実装

### 7.1 システムフォント（REQ-D03）

- `font-family: var(--font-sans)` を `html, body` に指定し、Web フォントは同梱しない。
- `-webkit-font-smoothing: antialiased` と `text-rendering: optimizeLegibility` を指定する（DS の base.css）。

### 7.2 ダーク／ライトモードへの追従（REQ-D04）

| 対象 | 方式 |
|---|---|
| Web コンテンツ | DS のトークンが `@media (prefers-color-scheme: dark)` で切り替わる。`color-scheme` プロパティでフォーム部品とスクロールバーも追従する |
| ネイティブのウィンドウ | `tauri.conf.json` でテーマを固定しない（OS の外観に追従）。vibrancy と信号機ボタンも自動で追従する |
| 手動指定（任意） | 設定の「外観」（既定は「自動」）で「ライト」「ダーク」を選んだ場合は、`<html data-theme>` を設定し、`getCurrentWindow().setTheme()` でウィンドウの外観も合わせる。「自動」に戻すときは `data-theme` を外し `setTheme(null)` を呼ぶ |
| 起動時のちらつき防止 | 手動指定がある場合は、ウィンドウ生成時に Rust 側でテーマを指定し、`initialization_script` で `data-theme` を先に設定する |

### 7.3 カーソルとテキスト選択（REQ-D05）

DS の `base.css` を適用する。

```css
html, body { cursor: default; -webkit-user-select: none; user-select: none; }
button, [role="button"], a, label, summary { cursor: default; }
input, textarea, [contenteditable="true"], .selectable, .s3-selectable {
  -webkit-user-select: text; user-select: text; cursor: text;
}
img, svg { -webkit-user-drag: none; }
```

| 項目 | 方針 |
|---|---|
| 選択できる箇所 | 入力欄、インスペクタのファイル名・キー・ETag・ARN・バージョン ID・Content-Type、エラーの詳細 |
| 選択できない箇所 | リストの行、ボタン、メニュー、ツールバー、サイドバー（範囲選択・ドラッグ操作と衝突するため） |
| ボタンのカーソル | shadcn/ui の初期化で `--no-pointer` を指定し、`cursor: pointer` を付けない |
| 右クリック | WebView 既定のコンテキストメニュー（「再読み込み」など）を無効にし、アプリのメニューだけを出す。入力欄ではカット／コピー／ペーストのため既定のメニューを残す |

### 7.4 透過タイトルバー（REQ-D06）

`src-tauri/tauri.conf.json` のメインウィンドウ設定（値は DS の Implementation notes に合わせる）:

```json
{
  "app": {
    "macOSPrivateApi": true,
    "windows": [
      {
        "label": "main",
        "title": "S3 Drive",
        "width": 1200,
        "height": 760,
        "minWidth": 720,
        "minHeight": 480,
        "titleBarStyle": "Overlay",
        "hiddenTitle": true,
        "trafficLightPosition": { "x": 19, "y": 24 },
        "transparent": true,
        "windowEffects": { "effects": ["sidebar"], "state": "followsWindowActiveState" },
        "visible": false
      }
    ]
  }
}
```

| 項目 | 方針 |
|---|---|
| ツールバー | 高さ 52px のツールバーがタイトルバーを兼ねる。ツールバー要素と余白の要素に `data-tauri-drag-region` を付け、ボタンなど操作部品には付けない（ドラッグは属性を持つ要素そのものを押したときだけ始まる） |
| 信号機ボタン | サイドバー上部の 52px を空けておく。サイドバーのない画面（サインイン）はツールバーの左に 88px の余白をとる |
| `transparent` | macOS では `macOSPrivateApi: true` が必要。この設定を使うアプリは Mac App Store に提出できないが、配布は GitHub Releases のため問題ない |
| 初回表示 | `visible: false` で生成し、最初の描画が終わってから `show()` する（白い画面のちらつきを防ぐ） |
| 位置の確認 | 信号機ボタンの位置とツールバーの縦位置は実機で確認し、必要に応じて `trafficLightPosition` を調整する |

### 7.5 マテリアル（vibrancy）

- ウィンドウ全体にネイティブの vibrancy（`sidebar` マテリアル）を効かせ、Web 側は `html, body` の背景を透明にする。
- メイン領域とインスペクタは `--background` で不透明に塗る。サイドバーは半透明の `--sidebar` を重ねて、下のネイティブ vibrancy を透かす。
- CSS の `backdrop-filter` はウィンドウの背後（デスクトップ）をぼかせないため、DS のモックでサイドバーに指定されている CSS のぼかしは使わず、ネイティブの vibrancy に置き換える。メニューとトーストは Web コンテンツの上に重なるため、CSS のぼかし（`material` ユーティリティ）を使う。
- `state: followsWindowActiveState` により、非アクティブ時は Finder と同じく平坦な表示になる。

### 7.6 その他のネイティブらしさ

| 項目 | 方針 |
|---|---|
| スクロールバー | DS の `::-webkit-scrollbar` 指定（幅 10px、角丸のつまみ）を使う |
| フォーカス | `:focus-visible` のときだけ 3px のリング（`--ring` 40%）。入力欄は枠も `--ring` |
| モーション | 120／200／320ms、`--ease-out`。`prefers-reduced-motion: reduce` のときは遷移を無効にする |
| メニューバー | macOS のアプリメニューは Tauri の Menu API でネイティブに作る（[03 §9.4](03-screens.md#94-アプリのメニューバー)）。コンテキストメニューは DS の Menu で描画する |
| Finder からのドロップ | Tauri の `onDragDropEvent` でファイルパスを受け取る（HTML の File オブジェクトからはパスが得られないため） |
| ズーム | ページのズーム（⌘＋／⌘−）は無効のままにする |

## 8. アイコン

### 8.1 アイコンセット

- `lucide-react` を使い、線幅 1.75、角丸キャップ、`currentColor` とする。絵文字と Unicode 記号は使わない（キーボードショートカットの ⌘ ⇧ ⌥ ⌫ は例外）。
- 主に使うグリフ: `folder`、`folder-plus`、`folder-open`、`folder-input`、`file`、`file-text`、`image`、`film`、`music`、`file-spreadsheet`、`file-archive`、`file-code`、`upload`、`download`、`cloud-upload`、`trash-2`、`search`、`search-x`、`sliders-horizontal`、`history`、`rotate-ccw`、`layers`、`hard-drive`、`globe`、`database`、`receipt`、`chart-pie`、`list`、`layout-grid`、`panel-right`、`settings`、`key-round`、`log-out`、`lock`、`circle-check`、`clock`、`refresh-cw`、`chevron-*`、`chevrons-up-down`、`plus`、`check`。

### 8.2 ファイル種別の判定

`DS: components/data/FileIcon.jsx` と同じ対応にする（`src/lib/file-kind.ts`）。

| 種別 | 拡張子 | グリフ | 色 | 種類の表示名 |
|---|---|---|---|---|
| folder | （フォルダ） | `folder` | `--ft-folder` | フォルダ |
| image | jpg, jpeg, png, gif, heic, webp, svg | `image` | `--ft-image` | 画像 |
| video | mp4, mov | `film` | `--ft-video` | ムービー |
| audio | mp3, wav, m4a | `music` | `--ft-audio` | オーディオ |
| pdf | pdf | `file-text` | `--ft-pdf` | PDF 書類 |
| sheet | csv, xlsx, numbers | `file-spreadsheet` | `--ft-sheet` | スプレッドシート |
| archive | zip, gz, tar | `file-archive` | `--ft-archive` | アーカイブ |
| code | js, ts, json, py, rs, html | `file-code` | `--ft-code` | ソースコード |
| doc | 上記以外 | `file` | `--ft-doc` | 書類 |

### 8.3 アプリアイコンとメニューバーアイコン

| 素材 | 用途 | 生成方法 |
|---|---|---|
| `DS: assets/app-icon.svg` | アプリアイコン（1024px、macOS のグリッド 824px、角丸 185px） | 1024px の PNG に書き出し、`pnpm tauri icon` で `.icns` などを生成する |
| `DS: assets/app-icon-dark.svg` | ダーク外観用のアイコン | macOS の外観別アイコン（Icon Composer）への対応は Tauri の対応状況を確認して判断する |
| `DS: assets/menubar-glyph.svg` | メニューバー常駐アイコン（単色、22pt） | テンプレート画像（`icon_as_template(true)`）として使い、OS が明暗に合わせて着色する |

ワードマークは独立したロゴを持たず、「S3 Drive」をシステムフォントの Bold で組む。

## 9. 文言と書式

### 9.1 文言ルール（DS の Content Fundamentals）

| 項目 | ルール | 例 |
|---|---|---|
| 言語 | 日本語 UI。AWS の固有名詞は英語のまま | Standard-IA、Glacier、ETag、ARN、ap-northeast-1 |
| トーン | macOS 標準アプリと同じく簡潔・丁寧（です／ます）。主語「あなた」、感嘆符、絵文字は使わない | |
| ボタン | 動詞のみ | アップロード、ダウンロード、移動、削除、作成、変更、接続 |
| ダイアログを開く項目 | 末尾に「…」 | 移動…、ストレージクラスを変更… |
| 確認ダイアログ | タイトルは対象を明示した疑問形。本文で結果を 1 文で説明する | 「report-q3.pdf」を削除しますか？／削除マーカーが作成されます。以前のバージョンからいつでも復元できます。 |
| 完了通知 | 過去形。進行中は「〜中」 | アップロードが完了しました／3 項目を移動しました／3 件をアップロード中 |
| 空状態・ヒント | 次に何をすればよいかを一言で | ファイルをドロップしてアップロード／一致する項目はありません |

### 9.2 書式

| 対象 | 書式 | 例 |
|---|---|---|
| サイズ | 10 進接頭辞（1 KB = 1,000 B、Finder と同じ）。100 以上は整数、未満は小数 1 桁。数字と単位の間は半角スペース | 4.2 MB、248.6 GB、842 MB |
| サイズ（詳細） | 併せてバイト数を桁区切りで表示する | 4.2 MB（4,200,000 バイト） |
| 日時 | ローカル時刻で `YYYY/MM/DD HH:mm` | 2026/09/27 14:32 |
| 件数 | 数字＋半角スペース＋「項目」または「件」 | 12 項目、3 件 |
| 金額 | 米ドル。1 ドル以上は小数 2 桁、1 ドル未満は小数 3 桁。0 より大きく 0.001 ドル未満は「$0.000」と区別できるよう有効数字 2 桁 | $6.94、$0.315、$0.00029 |
| 割合 | 小数 1 桁 | 72.6% |
| リージョン | コードと日本語名を併記し、短縮名も持つ | ap-northeast-1／アジアパシフィック (東京)／東京 |
| ストレージクラス | DS の `STORAGE_CLASSES` の表示名（英語） | Standard、Glacier Instant Retrieval（短縮: Glacier IR） |

## 10. アクセシビリティ

| 項目 | 方針 |
|---|---|
| アクセシブル名 | アイコンだけのボタンには必ず `label` を付ける（DS の IconButton の必須項目） |
| ファイル一覧 | リスト表示・アイコン表示とも複数選択の `role="listbox"`（`aria-multiselectable`、名前はバケット名）とし、各項目を `role="option"`（`aria-selected`）にする。リスト表示は仮想化で画面外の行を描画しないため、`aria-setsize`／`aria-posinset` で全体の件数と位置を示す。列の見出し（並べ替え）はリストの外に置く。キーボード操作は [03 §10](03-screens.md#10-キーボードショートカット) |
| ダイアログ | Base UI のフォーカストラップと、閉じたときのフォーカス復帰を使う |
| 通知 | トーストは `aria-live="polite"`、エラーは `assertive` |
| 色だけに頼らない | ストレージクラスや状態は色に加えて名前・アイコンで示す |
| コントラスト | DS の配色（ステータス文字色の `color-mix`）を守る。ダークモードも同じ基準で確認する |
| 検証 | VoiceOver での操作確認を手動テストに含める（[09-testing.md](09-testing.md)） |

# S3 Drive — Design System

**S3 Drive** は、事前に用意した Amazon S3 バケットを Google ドライブのように扱える macOS ネイティブアプリ。
このプロジェクトはそのアプリアイコン、デザイントークン、React コンポーネント、UI キットをまとめたもの。

## Product context
- **Stack**: Tauri v2 (Rust backend + AWS SDK for Rust) / Vite + React + TypeScript / shadcn/ui (Tailwind) on Base UI.
- **前提**: S3 バケットと IAM ユーザー／ロールは AWS 側で作成済み。アプリではバケット名・リージョン・アクセスキー・(任意) ロール ARN を入力する。
- **機能**: Google 認証、利用容量／リージョン／ストレージクラス表示、ストレージクラス変更、ファイル一覧・アップロード・ダウンロード・削除、コスト表示、メタデータ表示、検索（名前・拡張子・サイズ・更新日・クラス）、フォルダ作成・削除・移動、バージョン管理（表示・復元・削除）。
- **Sources**: ユーザーのブリーフのみ（コードベース・Figma・既存ロゴなし）。デザインはゼロから作成。

## Index
- `styles.css` — エントリポイント（@import のみ）
- `tokens/` — colors.css（パレット + shadcn 互換セマンティック + ライト/ダーク）, typography.css, spacing.css（余白・角丸・サイズ・モーション・マテリアル）, base.css（ネイティブ風のベースルール）
- `components/` — core (Icon, Button, IconButton) · forms (Input, Select, Checkbox, Switch, SegmentedControl) · feedback (Badge, StorageClassBadge, Progress, Toast, Tooltip, Dialog) · navigation (Menu, SidebarItem/SidebarSection, Breadcrumbs) · data (FileIcon) · shell (AppWindow/TrafficLights) · components.css
- `ui_kits/s3-drive/` — インタラクティブなアプリ画面（index.html / signin.html）
- `guidelines/` — ファウンデーション見本カード
- `assets/` — app-icon.svg, app-icon-dark.svg, menubar-glyph.svg
- `ds-resolve.js` — 見本/キット用ローダー（バンドルがなければ JSX をブラウザでコンパイル）
- `SKILL.md`

## CONTENT FUNDAMENTALS
- **言語**: 日本語 UI。AWS の固有名詞（Standard-IA, Glacier, ETag, ARN, リージョンコード）は英語のまま。
- **トーン**: macOS 標準アプリと同じく簡潔・丁寧（です/ます）。主語「あなた」は使わない。感嘆符なし、絵文字なし。
- **ボタン**: 動詞のみ — 「アップロード」「ダウンロード」「移動」「削除」「作成」「変更」「接続」。ダイアログを開くメニュー項目は末尾に「…」（例: 「移動…」「ストレージクラスを変更…」）。
- **確認ダイアログ**: タイトルは疑問形で対象を明示 — 「『report-q3.pdf』を削除しますか？」。本文で結果を1文で説明 — 「削除マーカーが作成されます。以前のバージョンからいつでも復元できます。」
- **完了通知**: 過去形 — 「アップロードが完了しました」「3 項目を移動しました」。進行中は「〜中」。
- **数値**: 半角数字 + 半角スペース + 単位（4.2 MB, 248.6 GB, $6.94）。件数は「12 項目」。日付は `2026/09/27 14:32`。数字には tabular-nums。
- **空状態/ヒント**: 何をすればよいかを一言で — 「ファイルをドロップしてアップロード」「一致する項目はありません」。

## VISUAL FOUNDATIONS
- **全体の方針**: 「Finder の隣に置いて違和感のないアプリ」。装飾ではなく macOS のマテリアル・密度・操作感に合わせる。ブランド色は Azure 1 色のみで、選択・主要アクション・フォーカスに使う。
- **色**: クールグレー（hue 250, 彩度 ≤ 0.012）+ Azure アクセント（hue 240）。トークンは shadcn/ui 互換名（--background, --primary, --muted-foreground …）で Tailwind にそのままマップできる。ストレージクラスとファイル種別は同じ L/C で色相だけを変えた識別色（--sc-*, --ft-*）。ステータス色（success/warning/destructive）のテキストは `color-mix` で foreground 寄りに寄せてコントラストを確保。
- **ライト/ダーク**: `prefers-color-scheme` に自動追従。`[data-theme]` / `.dark` で強制も可。ダークでは境界線は白の半透明、影は濃く、上端に 0.5px のハイライト。
- **タイポグラフィ**: `-apple-system`（SF Pro）→ 日本語は Hiragino Sans。本文 13px が基準（macOS HIG）。見出しは 15/17/22px の semibold〜bold、キャプション 11px。キー・ETag・ARN・バージョン ID は SF Mono 11–12px。
- **余白/密度**: 4px グリッド（+2/6px）。コントロール高 24/28/32px、リスト行 28px。サイドバー 220px、インスペクタ 280px、タイトルバー 52px。
- **角丸**: 4（チェック）/5（バッジ）/6（ボタン・入力・行）/8（メニュー・カード）/10（トースト・カード）/12（ダイアログ・ウィンドウ）。大きな丸みは使わない。
- **ボーダー/影**: 0.5px のヘアラインが基本。影は「0.5px の輪郭リング + 柔らかいドロップ」の組み合わせ（--shadow-xs〜lg）。カードは面の差 + shadow-sm、左ボーダーのアクセントは使わない。
- **マテリアル/透過**: タイトルバーは透過（ツールバーがタイトルバーを兼ねる、drag region）。サイドバー・メニュー・トーストのみ vibrancy（半透明 + `saturate(180%) blur(24px)`）。コンテンツ面は不透明。
- **背景**: 画像・グラデーション・パターンは使わない。アプリアイコンのみグラデーション。プレビュー未取得領域はストライプのプレースホルダー。
- **ホバー/押下**: ホバーは背景をわずかに濃く（foreground/black への color-mix 6–10%）。押下はさらに濃く。拡大縮小はしない（ネイティブに合わせる）。メニュー項目はホバーで primary 塗り + 白文字（macOS 準拠）。
- **選択**: 行はインセットの角丸 6px、primary 塗り + 白文字。アイコン表示はアイコン背景を淡く、名前をピル状に primary。
- **フォーカス**: 3px の ring（--ring 40%）。入力はボーダーも --ring に。
- **モーション**: 120ms（ホバー・色）/ 200ms（スイッチ・トグル）/ 320ms（プログレス）。`cubic-bezier(0.2,0,0,1)`。バウンスなし、フェード主体。
- **カーソル/選択**: 全体 `cursor: default` + `user-select: none`。入力欄と `.s3-selectable`（ファイル名、キー、ETag、ARN、バージョン ID）だけテキスト選択可。
- **レイアウト固定要素**: ツールバー（上）、ステータスバー（下 26px）、サイドバー（左）、インスペクタ（右）。トーストは右下スタック。ダイアログはウィンドウ内オーバーレイ。

## ICONOGRAPHY
- **アイコンセット**: [Lucide](https://lucide.dev)（shadcn/ui 標準）。CDN `lucide@0.460.0` UMD を `<Icon name="…">` で描画。本番では `lucide-react` を使用。コードベースがないためこれが代替ではなく正式採用。
- **スタイル**: 線幅 1.75、角丸キャップ、`currentColor`。サイズはツールバー/行 16px、小ボタン 14px、バッジ 11–12px、空状態 32px。
- **ファイル種別**: FileIcon が拡張子から Lucide グリフ + --ft-* 色 + 22% の塗りで表示。フォルダは Azure。
- **絵文字・Unicode 記号**: 使わない（キーボードショートカット表記 ⌘ ⇧ ⌫ のみ例外）。
- **アプリアイコン**: `assets/app-icon.svg`（1024、macOS グリッド 824px / 角丸 185px）。Azure グラデーションの上に白いバケットとアップロード矢印。ダーク版 `app-icon-dark.svg`。メニューバー用テンプレート画像 `menubar-glyph.svg`（単色、22pt）。本番では Icon Composer / `tauri icon` で .icns を生成。
- **ロゴ**: 独立したロゴはなし。ワードマークは「S3 Drive」をシステムフォント Bold で組む。

## Implementation notes (Tauri v2)
```jsonc
// tauri.conf.json → app.windows[0]
{ "titleBarStyle": "Overlay", "hiddenTitle": true, "transparent": true,
  "trafficLightPosition": { "x": 19, "y": 24 },
  "windowEffects": { "effects": ["sidebar"], "state": "followsWindowActiveState" } }
// app.macOSPrivateApi: true（transparent に必要）
```
- ツールバーに `data-tauri-drag-region` を付与。背景透過のため `html, body` の background はサイドバー部分を透明にする。
- Tailwind v4: `@theme inline { --color-background: var(--background); --color-primary: var(--primary); … }` で tokens/colors.css をマップ。`--radius` は 6px を基準に。

## Intentional additions
- **StorageClassBadge / FileIcon / AppWindow** — S3 とmacOS ウィンドウ固有の表現のため標準セットに追加。

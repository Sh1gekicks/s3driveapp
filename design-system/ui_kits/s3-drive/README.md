# S3 Drive — macOS app UI kit

Click-through mock of the Tauri app. Open `index.html` (main window) or `signin.html` (onboarding).

| File | Surface |
|---|---|
| SignIn.jsx | Google サインイン → IAM 認証情報 + バケット接続 |
| Sidebar.jsx | ドライブ / バケット / 利用容量メーター / アカウント |
| Toolbar.jsx | 透過タイトルバー内ツールバー（戻る/進む・パンくず・検索・フィルタ・表示切替・新規フォルダ・アップロード）+ FilterBar |
| FileList.jsx | リスト（列ソート）/ アイコン表示、ドラッグ&ドロップアップロード、ステータスバー |
| Inspector.jsx | 詳細（メタデータ・ストレージクラス）/ バージョン（復元・DL・削除） |
| Dashboard.jsx | 利用容量・リージョン・クラス別内訳・コスト内訳・日別コスト |
| Dialogs.jsx | 新規フォルダ・削除・移動・ストレージクラス変更 |
| App.jsx | 状態管理とイベント配線 |
| data.js | モックデータ（2 バケット、バージョン、料金） |

Interactions: ダブルクリックでフォルダを開く / ⌘・⇧クリックで複数選択 / 右クリックでコンテキストメニュー / ⌘⌫ 削除 / ⌘A 全選択 / Finder からファイルをドロップ、または「アップロード」で実ファイル選択 / 外観切替（自動・ライト・ダーク）。

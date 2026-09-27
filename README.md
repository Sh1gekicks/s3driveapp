# S3 Drive

S3 バケットを、いつものドライブのように扱える macOS アプリ。Tauri 2（Rust）と React で作られています。
設計書は [docs/design](docs/design/README.md)、画面の見た目の正本は [design-system](design-system/readme.md) です。

## 構成

| パス | 内容 |
|---|---|
| `crates/s3drive-core` | Tauri に依存しないコア（S3・STS・CloudWatch・Cost Explorer・Price List、SQLite、キーチェーン、転送、検索） |
| `src-tauri` | アプリ本体（パッケージ名 `s3drive-app`）。IPC コマンド、メニュー、メニューバー常駐、ウィンドウ |
| `src` | フロントエンド（React 19、TypeScript 7、Tailwind CSS 4、Base UI、TanStack Query、Zustand） |
| `src/lib/ipc/bindings` | ts-rs が Rust の型から生成した TypeScript の型（手で編集しない） |
| `e2e` | WebdriverIO の E2E テスト |

## 必要なもの

- macOS 13 以降（アプリの実行）。コアのテストは Linux でも実行できます
- Rust（stable。`rust-toolchain.toml`）
- pnpm 12（`packageManager`）と Node.js 26（`.node-version`）
- Docker（結合テストの S3 モック moto）

## 開発

```sh
pnpm install

# モックバックエンドでフロントエンドだけを表示する（ブラウザ）
pnpm dev:mock                  # http://localhost:1420/ 、?signedOut=1 でサインイン前、?empty=1 で接続なし

# アプリを起動する（Google OAuth のクライアントが未設定の開発ビルドは固定のセッションでサインインする）
pnpm tauri dev
```

S3 の代わりに moto に接続する場合:

```sh
docker compose -f docker-compose.test.yml up -d
S3DRIVE_TEST_ENDPOINT=http://localhost:5000 pnpm tauri dev
```

## テストと検査

```sh
pnpm biome ci .                # 書式・静的解析
pnpm typecheck                 # tsc -b
pnpm test                      # Vitest（単体テストとモックバックエンドを使う画面テスト）

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p s3drive-core     # S3DRIVE_TEST_ENDPOINT を設定すると moto を使う結合テストも実行する
cargo deny check

# E2E（macOS。moto が S3DRIVE_TEST_ENDPOINT で動いていること）
pnpm e2e:build && pnpm e2e
```

E2E はバージョニングを有効にしたバケット（既定は `s3drive-e2e`。`S3DRIVE_E2E_BUCKET` で変更できる）を使います。moto を起動したら、先に作っておいてください。

```sh
curl -sf -X PUT http://localhost:5000/s3drive-e2e
curl -sf -X PUT -H 'Content-Type: application/xml' 'http://localhost:5000/s3drive-e2e?versioning' --data-binary @e2e/fixtures/versioning.xml
```

macOS では AirPlay レシーバーが 5000 番を使っているため、そのままでは moto に接続できません（`403 Forbidden` が返ります）。システム設定の「一般 > AirDrop と Handoff」で AirPlay レシーバーをオフにするか、moto を別のポートで起動して `S3DRIVE_TEST_ENDPOINT` を合わせてください。

```sh
docker run -d --rm -p 5055:5000 motoserver/moto:5.1.22
S3DRIVE_TEST_ENDPOINT=http://localhost:5055 pnpm e2e
```

Rust の型を変更したら `cargo test -p s3drive-core` で `src/lib/ipc/bindings` を書き出し、差分をコミットしてください（CI で差分を確認します）。

## リリース

`package.json` の `version` を更新してマージし、`vX.Y.Z` のタグを push すると、`release.yml` が署名・公証済みのビルドを下書きのリリースとして作ります（[08 §4](docs/design/08-cicd.md#4-releaseyml)）。
必要なシークレットは [08 §5](docs/design/08-cicd.md#5-シークレットと変数) のとおりです。加えて、自動更新の公開鍵をリポジトリの変数 `TAURI_UPDATER_PUBKEY` に設定してください（`pnpm tauri signer generate` で作成。リリース時に `tauri.conf.json` へ重ねて設定します）。

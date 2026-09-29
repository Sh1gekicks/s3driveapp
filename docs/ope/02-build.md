# 02. ビルドの方法

ビルドの成果物は Cargo ワークスペース直下の `target/` に出力される（`src-tauri/target/` ではない）。フロントエンドは `dist/` に出力され、アプリに埋め込まれる。

## 1. ローカルでのビルド

事前に [01 ローカル開発環境の構築](01-local-setup.md) を済ませておく。

### 1.1 ビルドの種類

| 種類 | コマンド | 成果物 | 用途 |
|---|---|---|---|
| フロントエンドのみ | `pnpm build` | `dist/` | フロントエンドのビルドエラーの確認 |
| デバッグビルド | `pnpm tauri build --debug --bundles app` | `target/debug/bundle/macos/S3 Drive.app` | CI の `app` ジョブと同じビルド。動作確認 |
| リリースビルド（手元の CPU 向け） | `pnpm tauri build` | `target/release/bundle/macos/S3 Drive.app`、`target/release/bundle/dmg/S3 Drive_<版>_<CPU>.dmg` | 配布物に近い形での確認 |
| リリースビルド（ユニバーサル） | `pnpm tauri build --target universal-apple-darwin` | `target/universal-apple-darwin/release/bundle/` 以下の `.app` と `.dmg` | リリースと同じ構成（Apple Silicon と Intel の両方）での確認 |
| E2E 用ビルド | `pnpm e2e:build` | `target/debug/s3drive-app` | E2E テスト（[03 §2.5](03-test.md#25-e2e-テスト)） |

- `pnpm tauri build` は、`tauri.conf.json` の `beforeBuildCommand`（`pnpm build`）でフロントエンドをビルドしてから Rust をビルドし、バンドルを作る。
- ユニバーサルビルドに必要なターゲット（`aarch64-apple-darwin`、`x86_64-apple-darwin`）は `rust-toolchain.toml` で導入済み。2 つのターゲットをビルドするため時間がかかる。

### 1.2 署名

`tauri.conf.json` の `signingIdentity: "-"` により、ローカルのビルドでもバンドラが Hardened Runtime 付きのアドホック署名を行う。証明書は不要。署名は次のコマンドで確認できる。

```bash
codesign --verify --deep --strict --verbose=2 "target/release/bundle/macos/S3 Drive.app"
```

```bash
codesign --display --verbose=2 "target/release/bundle/macos/S3 Drive.app"
```

出力に `Signature=adhoc` と、`flags` に `runtime` が含まれていればよい。環境変数 `APPLE_SIGNING_IDENTITY` を設定していると、そちらが優先されるので注意する。

### 1.3 リリースビルドでの Google サインイン

リリースビルドは、Google の OAuth クライアントがビルド時に埋め込まれていないとサインインできない（開発ビルドのような固定のセッションにはならず、「Google の OAuth クライアントが設定されていないため、サインインできません」と表示される）。リリースビルドを手元で動かして確認する場合は、テスト用の OAuth クライアントを環境変数で渡してビルドする（[01 §5](01-local-setup.md#5-google-サインインを使う場合任意)）。

```bash
S3DRIVE_GOOGLE_CLIENT_ID=<クライアント ID> S3DRIVE_GOOGLE_CLIENT_SECRET=<シークレット> pnpm tauri build
```

- リリースビルドでは `S3DRIVE_TEST_ENDPOINT` が無視されるため、moto には接続できない。
- リリースビルドは、リリース版と同じ保存先（`~/Library/Application Support/io.github.sh1gekicks.s3drive/`）とキーチェーンを使う。リリース版をインストールしている Mac では、データを共有することに注意する。

### 1.4 自動更新の成果物

`tauri.conf.json` では `createUpdaterArtifacts` を `false` にしているため、ローカルのビルドでは自動更新用の成果物（`.app.tar.gz` と署名）は作られず、署名の鍵も不要。自動更新の成果物はリリースのワークフローだけで作る（[04 §3](04-release.md#3-releaseyml-の処理)）。

### 1.5 ビルドしたアプリの起動

ローカルでビルドしたアプリには quarantine 属性が付かないため、Gatekeeper に止められずに起動できる。

```bash
open "target/release/bundle/macos/S3 Drive.app"
```

### 1.6 クリーンビルド

依存の更新後などにビルドがおかしくなった場合は、成果物を削除してからビルドし直す。

```bash
cargo clean && rm -rf dist
```

## 2. GitHub Actions でのビルド

### 2.1 CI（ci.yml）

PR の作成・更新時と、main への push 時に `.github/workflows/ci.yml` が実行され、ビルドが通ることを確認する。テストの内容は [03 §3](03-test.md#3-github-actions-でのテスト) を参照。

| ジョブ | ランナー | ビルドの内容 |
|---|---|---|
| `frontend` | ubuntu-latest | `pnpm vite build`（フロントエンドのビルド） |
| `core` | ubuntu-latest | `cargo test -p s3drive-core --locked`（`s3drive-core` のビルドとテスト） |
| `app` | macos-latest | `pnpm tauri build --debug --bundles app`（macOS 向けのデバッグビルド。アドホック署名を含む） |

- `app` ジョブは、実行コストの高い macOS ランナーを使うため、`frontend` と `core` が成功してから実行する。
- CI のビルド結果（`.app`）は成果物としてアップロードしない。配布物が必要な場合はリリースのワークフローを使う（[04](04-release.md)）。
- 同じブランチで新しい push があると、実行中のワークフローはキャンセルされる（`concurrency`）。
- Cargo のビルド結果（`Swatinem/rust-cache`）をキャッシュしている。pnpm のストアはキャッシュしない（理由は [設計 08 §9](../design/08-cicd.md#9-実行時間とコスト)）。目安の実行時間は `frontend` 約 2 分、`core` 約 3〜6 分、`app` 約 10 分。
- すべて `--locked`（Cargo）と `require-lockfile: true`（pnpm）で実行する。ロックファイルとの食い違いはエラーになるため、依存を変えたら `Cargo.lock` と `pnpm-lock.yaml` もコミットする。

### 2.2 リリースのビルド（release.yml）

タグ `vX.Y.Z` の push で `.github/workflows/release.yml` が実行され、ユニバーサルバイナリのビルド、アドホック署名、下書きのリリースの作成まで行う。手順は [04 リリースの方法](04-release.md) を参照。

### 2.3 実行結果の確認

GitHub の「Actions」タブ、または GitHub CLI で確認する。

```bash
gh run list --workflow ci.yml --branch <ブランチ名>
```

```bash
gh run view <実行 ID> --log-failed
```

失敗したジョブを再実行する場合:

```bash
gh run rerun <実行 ID> --failed
```

### 2.4 よくある失敗

| 失敗箇所 | 原因と対処 |
|---|---|
| `pnpm/setup`（install） | `pnpm-lock.yaml` が `package.json` と合っていない。手元で `pnpm install` してロックファイルをコミットする |
| `cargo ... --locked` | `Cargo.lock` の更新が必要な変更がある。手元で `cargo build` してロックファイルをコミットする |
| `pnpm vite build` | 手元で `pnpm build` を実行して再現させる |
| `pnpm tauri build --debug --bundles app` | macOS 固有のビルドエラー。手元の Mac で同じコマンドを実行して再現させる |

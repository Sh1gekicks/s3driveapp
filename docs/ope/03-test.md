# 03. テストの方法

テストの戦略と観点は [09 テスト設計](../design/09-testing.md) を参照する。本書は実行の手順をまとめる。

| 種類 | 対象 | ツール | 置き場所 | ローカル | CI |
|---|---|---|---|---|---|
| 書式・静的解析 | TypeScript、JSON、CSS | Biome | — | ○ | `frontend` |
| 型検査 | TypeScript | `tsc -b`（TypeScript 7） | — | ○ | `frontend` |
| フロントエンドのテスト | 単体テスト、モックバックエンドを使う画面テスト | Vitest（jsdom） | `src/**/*.test.{ts,tsx}` | ○ | `frontend` |
| 書式・静的解析 | Rust | rustfmt、clippy | — | ○ | `core`、`app` |
| Rust の単体テスト | `s3drive-core`、`s3drive-app` | `cargo test` | 各モジュールの `#[cfg(test)]` | ○ | `core`、`app` |
| Rust の結合テスト | `s3drive-core` と S3 モック | `cargo test` + moto | `crates/s3drive-core/tests/moto.rs` | ○（Docker） | `core` |
| 依存の検査 | ライセンス・脆弱性 | cargo deny、pnpm audit | `deny.toml` | ○ | `audit` |
| E2E テスト | アプリ全体と S3 モック | WebdriverIO + moto | `e2e/specs/` | ○（macOS） | `e2e.yml` |

## 1. ローカルで CI と同じ検査を行う

PR を作る前に、CI と同じ検査を手元で実行する。上から順に速い。

```bash
pnpm biome ci .
```

```bash
pnpm typecheck
```

```bash
pnpm test
```

```bash
cargo fmt --all --check
```

```bash
cargo clippy --workspace --all-targets --locked -- -D warnings
```

```bash
cargo clippy -p s3drive-app --all-targets --locked --features e2e -- -D warnings
```

```bash
cargo test --workspace --locked
```

```bash
git diff --exit-code -- src/lib/ipc/bindings
```

- `cargo test` は ts-rs の型定義（`src/lib/ipc/bindings`）を書き出す。最後の `git diff` で差分が出た場合は、書き出された型定義をコミットする。
- 書式の違反は、`pnpm biome check --write .` と `cargo fmt --all` で自動修正できる。

## 2. ローカルでのテスト

### 2.1 フロントエンド（Vitest）

```bash
pnpm test
```

| 目的 | コマンド |
|---|---|
| 変更を監視して再実行する | `pnpm vitest` |
| 特定のファイルだけ実行する | `pnpm vitest run src/lib/format.test.ts` |
| テスト名で絞る | `pnpm vitest run -t "<テスト名の一部>"` |
| カバレッジを取る（CI と同じ） | `pnpm vitest run --coverage`（結果は `coverage/`） |

- 画面テストは、`pnpm dev:mock` と同じモックバックエンド（`src/lib/ipc/mock`）を使う。Rust のビルドは不要。
- カバレッジの対象は `src/lib`、`src/features`、`src/stores`（生成物とモックは除く）。

### 2.2 Rust（単体テストと moto の結合テスト）

単体テストだけを実行する場合（Docker 不要）:

```bash
cargo test -p s3drive-core
```

```bash
cargo test -p s3drive-app
```

`S3DRIVE_TEST_ENDPOINT` を設定すると、moto を使う結合テスト（`crates/s3drive-core/tests/moto.rs`）も実行する。未設定の場合、結合テストは「S3DRIVE_TEST_ENDPOINT が未設定のためスキップします」と表示して成功扱いになる。

```bash
docker compose -f docker-compose.test.yml up -d
```

```bash
S3DRIVE_TEST_ENDPOINT=http://localhost:5000 cargo test -p s3drive-core
```

結合テストだけを実行する場合:

```bash
S3DRIVE_TEST_ENDPOINT=http://localhost:5000 cargo test -p s3drive-core --test moto
```

- 結合テストはテストごとにバケットを作るため、事前のバケットの作成は不要。
- 終わったら moto を停止する: `docker compose -f docker-compose.test.yml down`
- 5000 番ポートで `403 Forbidden` になる場合は、AirPlay レシーバーが原因（[01 §7](01-local-setup.md#7-トラブルシューティング)）。

### 2.3 E2E テスト

`e2e` 機能を有効にしたアプリ（WebDriver サーバーを内蔵し、Google サインインを固定のセッションにしたもの）を moto に接続し、WebdriverIO で操作する。macOS でのみ実行できる。

**1. moto を起動し、バージョニングを有効にしたバケットを作る。**

```bash
docker compose -f docker-compose.test.yml up -d
```

```bash
curl -sf -X PUT http://localhost:5000/s3drive-e2e
```

```bash
curl -sf -X PUT -H 'Content-Type: application/xml' 'http://localhost:5000/s3drive-e2e?versioning' --data-binary @e2e/fixtures/versioning.xml
```

`Content-Type` を付けないと、moto がフォームとして扱い 404 を返す。バージョニングが有効になったことは次で確認できる（`Enabled` が含まれる）。

```bash
curl -sf 'http://localhost:5000/s3drive-e2e?versioning'
```

**2. アプリをビルドする。**

```bash
pnpm e2e:build
```

**3. テストを実行する。**

```bash
pnpm e2e
```

| 環境変数 | 既定値 | 内容 |
|---|---|---|
| `S3DRIVE_TEST_ENDPOINT` | `http://localhost:5000` | moto のエンドポイント |
| `S3DRIVE_E2E_BUCKET` | `s3drive-e2e` | テストに使うバケット（バージョニングを有効にしておく） |
| `S3DRIVE_E2E_BINARY` | `target/debug/s3drive-app` | テストするアプリのバイナリ |
| `S3DRIVE_E2E_ARTIFACTS` | `e2e/.artifacts` | 失敗したシナリオのスクリーンショットの保存先 |

- シナリオは前のシナリオの結果を使うため、1 つのセッションで順に実行し、失敗した時点で止まる。
- moto を 5000 番以外で動かす場合は、両方の環境変数を合わせる。

  ```bash
  docker run -d --rm -p 5055:5000 motoserver/moto:5.1.22
  ```

  ```bash
  S3DRIVE_TEST_ENDPOINT=http://localhost:5055 pnpm e2e
  ```

- アプリのコードを変更したら、`pnpm e2e:build` からやり直す。
- Docker を使わない場合は、CI と同じく Python で moto のサーバーを起動してもよい（`pip install 'moto[server]==5.1.22'` のあと `moto_server -H 127.0.0.1 -p 5000`）。

### 2.4 依存の検査

```bash
cargo install --locked cargo-deny
```

```bash
cargo deny check
```

```bash
pnpm audit --prod --audit-level high
```

- cargo deny は、ライセンス・脆弱性（RustSec）・重複・取得元を検査する。勧告を例外にする場合は、`deny.toml` に ID と理由を書く。
- AWS のアクセスキー ID の形の文字列がないことは、次で確認できる（CI の `audit` ジョブと同じ）。

  ```bash
  git grep -nE '(AKIA|ASIA)[0-9A-Z]{16}' -- . ':!design-system'
  ```

### 2.5 手動テスト

実際の AWS と Google での確認は、[09 §2.7](../design/09-testing.md#27-手動テスト) と [09 §3](../design/09-testing.md#3-テスト環境) の検証用の環境で行う。リリース前の確認は [04 §4.4](04-release.md#44-下書きのリリースを確認する) を参照。

## 3. GitHub Actions でのテスト

### 3.1 CI（ci.yml）

PR の作成・更新時と、main への push 時に自動で実行される。PR は 4 つのジョブがすべて成功してからマージする（設計では必須チェックとしているが、2026-09 時点のルールセット `main` には必須のステータスチェックを設定していない。[04 §1.4](04-release.md#14-リポジトリの設定)）。

| ジョブ | ランナー | 内容 |
|---|---|---|
| `frontend` | ubuntu-latest | `pnpm biome ci .`、`pnpm typecheck`、`pnpm vitest run --coverage`、`pnpm vite build` |
| `core` | ubuntu-latest（サービス: moto 5.1.22） | `cargo fmt --all --check`、`cargo clippy -p s3drive-core`、`cargo test -p s3drive-core`（`S3DRIVE_TEST_ENDPOINT` を設定し、結合テストも実行）、ts-rs の型定義の差分確認 |
| `app` | macos-latest（`frontend` と `core` の成功後） | `cargo clippy -p s3drive-app`（`e2e` 機能の有無の両方）、`cargo test -p s3drive-app`、デバッグビルド |
| `audit` | ubuntu-latest | アクセスキー ID の形の文字列の検査、`cargo deny check`、`pnpm audit --prod --audit-level high` |

- `core` ジョブは Linux で動くため、キーチェーンの代わりにメモリ上の実装でテストする。
- 失敗の確認と再実行は [02 §2.3](02-build.md#23-実行結果の確認) を参照。

### 3.2 E2E（e2e.yml）

実行コストの高い macOS ランナーを長く使うため、PR ごとには実行しない。

| トリガー | 実行の条件 |
|---|---|
| 毎晩 | 3:00 JST（`cron: '0 18 * * *'`）に main で実行 |
| 手動 | 「Actions」→「E2E」→「Run workflow」、または下の `gh` コマンド |
| PR | `e2e` ラベルが付いている PR で、ラベルを付けたときと、その後の push のたび |

```bash
gh workflow run e2e.yml --ref <ブランチ名>
```

PR で実行する場合は、PR に `e2e` ラベルを付ける。

```bash
gh pr edit <PR 番号> --add-label e2e
```

- ランナーには Docker がないため、moto は Python の仮想環境にインストールしてサーバーとして起動し、バケットを作成してから実行する。
- タイムアウトは 60 分。
- 失敗した場合は、実行結果の成果物 `e2e-logs` に、失敗したシナリオのスクリーンショット（`e2e/.artifacts/`）、moto のログ、アプリのログが保存される。

  ```bash
  gh run download <実行 ID> -n e2e-logs
  ```

### 3.3 失敗したときの対処

| 失敗箇所 | 原因と対処 |
|---|---|
| `biome ci` | 手元で `pnpm biome check --write .` を実行してコミットする |
| `cargo fmt --check` | 手元で `cargo fmt --all` を実行してコミットする |
| clippy | 警告もエラー扱い（`-D warnings`）。手元で同じコマンドを実行して直す |
| 型定義（ts-rs）の差分確認 | Rust の型を変えたのに型定義をコミットしていない。手元で `cargo test -p s3drive-core` を実行し、`src/lib/ipc/bindings` の差分をコミットする |
| アクセスキー ID の検査 | テストの値などに `AKIA`／`ASIA` で始まる 20 文字がある。`concat!("AKIA", "...")` のように分けて書く |
| `cargo deny` | 新しい勧告か、許可していないライセンス。依存を更新するか、影響がないことを確認して `deny.toml` に理由付きで例外を追加する |
| `pnpm audit` | 本番の依存に high 以上の脆弱性。依存を更新する（Renovate の PR を待つか、手動で更新する） |
| E2E | `e2e-logs` のスクリーンショットとログを確認し、手元で [§2.3](#23-e2e-テスト) の手順で再現させる |

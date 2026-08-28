# cff-test

CloudFront resource をデプロイしたり AWS 認証情報を設定したりせずに、AWS CloudFront Functions をローカルや CI からテストできます。

`cff-test` は、CloudFront Functions JavaScript runtime 2.0 の viewer request / viewer response 関数に対応するオフラインテスト runner です。

- AWS account や認証情報が不要
- CloudFront Function resource が不要
- GitHub Actions などの CI 環境で利用可能
- CloudFront Functions runtime との互換性を検証
- CloudFront KeyValueStore fixture に対応

QuickJS と対応する組み込み module は実行ファイルへ内包されるため、実行時に Node.js は必要ありません。

> [!WARNING]
> `cff-test` の capability 制限は、信頼できる関数コードの互換性テストを目的としています。信頼できない JavaScript を安全に実行するためのセキュリティ sandbox ではありません。

## モチベーション

CloudFront Functions は短いコードでも配信時の viewer request / viewer response に直接影響します。一方、変更のたびに AWS へデプロイして確認する方法では、フィードバックを得るまでに時間がかかり、開発環境や CI に AWS へのアクセス権限も必要になります。

`cff-test` は、関数コード、入力 event、期待する戻り値をリポジトリで一緒に管理し、通常のアプリケーションコードと同じ感覚でローカルや CI からテストするために作られました。pull request の段階で互換性違反や意図しない挙動を検出し、AWS へデプロイする前のフィードバックを短くすることを目指しています。

AWS 上での最終確認を置き換えるのではなく、日々の変更を気軽に繰り返し検証できる、高速で再現可能なテスト層を提供します。

## 比較

| | `cff-test` | AWS `test-function` | Node.js unit test |
| --- | --- | --- | --- |
| AWS account と認証情報 | 不要 | 必要 | 不要 |
| CloudFront Function resource | 不要 | 必要 | 不要 |
| オフライン実行 | 可能 | 不可 | 可能 |
| Runtime 互換性検査 | あり | Native runtime で実行 | なし |
| KeyValueStore のテスト | ローカル fixture | 関連付けた KVS | Mock が必要 |
| GitHub Actions での利用 | 可能 | 可能 | 可能 |

## `aws cloudfront test-function` ではなく cff-test を使う理由

AWS の [`test-function`](https://docs.aws.amazon.com/cloudfront/latest/APIReference/API_TestFunction.html) は、既存の CloudFront Function を AWS 上で実行します。Native runtime での検証に適していますが、CloudFront Function resource、AWS 認証情報、ネットワーク接続が必要です。

`cff-test` は、AWS resource を作成・更新するより前の段階での利用を想定しています。

```text
pull request -> cff-test -> merge -> AWS function を更新 -> AWS test-function -> publish
```

ローカルや CI での高速かつ繰り返し可能なフィードバックには `cff-test` を使い、native runtime 上の挙動を確認するときは AWS でテストしてください。

## 主な機能

- CloudFront Functions runtime 2.0 で利用できない構文、global、module、member を実行前に診断
- viewer request / viewer response event と handler の戻り値を検証
- 関数の戻り値を整形済み JSON として出力
- 期待値との差分を JSON Pointer 単位で表示
- `crypto`、`querystring`、ローカル KVS fixture を使う `cloudfront` module に対応
- `--now-ms` による `Date` の再現可能な固定
- AWS 認証情報、ネットワーク接続、Node.js が不要

対応範囲の詳細は [互換性ドキュメント](docs/compatibility.md) を参照してください。

## インストール

### GitHub Releases からダウンロード

[GitHub Releases](https://github.com/kaminchu/cff-test/releases) では、tag ごとに次の実行ファイルを配布します。ファイル名の `<TAG>` は、例えば `v0.1.0` です。

| 環境 | release asset |
| --- | --- |
| Linux x86（32-bit） | `cff-test-<TAG>-i686-unknown-linux-gnu` |
| Linux amd64 | `cff-test-<TAG>-x86_64-unknown-linux-gnu` |
| Linux arm64 | `cff-test-<TAG>-aarch64-unknown-linux-gnu` |
| macOS Intel | `cff-test-<TAG>-x86_64-apple-darwin` |
| macOS Apple Silicon | `cff-test-<TAG>-aarch64-apple-darwin` |

ダウンロードしたファイルには、必要に応じて実行権限を付け、`PATH` の通った場所へ配置してください。Linux amd64 の例:

```sh
chmod +x cff-test-v0.1.0-x86_64-unknown-linux-gnu
mkdir -p "$HOME/.local/bin"
install -m 0755 cff-test-v0.1.0-x86_64-unknown-linux-gnu "$HOME/.local/bin/cff-test"
```

配布ファイルは archive ではなく単一の実行ファイルです。macOS 向けバイナリのコード署名と notarization は行っていません。

### ソースコードからビルド

Rust toolchain と対象 platform の C/C++ build toolchain が必要です。release build と同じ Rust 1.96.0 を使う場合:

```sh
git clone https://github.com/kaminchu/cff-test.git
cd cff-test
rustup toolchain install 1.96.0 --profile minimal
cargo +1.96.0 build --locked --release
```

生成される実行ファイルは `target/release/cff-test` です。

## クイックスタート

URI を書き換える viewer request 関数を例に、静的検査、実行、期待値との比較を行います。

`function.js`:

```js
function handler(event) {
  event.request.uri = "/rewritten";
  return event.request;
}
```

`event.json`:

```json
{
  "version": "1.0",
  "context": { "eventType": "viewer-request" },
  "viewer": { "ip": "198.51.100.11" },
  "request": {
    "method": "GET",
    "uri": "/original",
    "querystring": {},
    "headers": { "host": { "value": "example.com" } },
    "cookies": {}
  }
}
```

`expected.json`:

```json
{
  "method": "GET",
  "uri": "/rewritten",
  "querystring": {},
  "headers": { "host": { "value": "example.com" } },
  "cookies": {}
}
```

```sh
# runtime 2.0 との互換性を静的検査
cff-test check function.js

# 関数を実行し、戻り値を標準出力へ表示
cff-test run function.js --event event.json

# 戻り値を expected.json と比較
cff-test test function.js --event event.json --expected expected.json
```

成功時、`check` は `OK: function.js is compatible with cloudfront-js-2.0`、`test` は `PASS: function.js` を出力します。

## GitHub Actions

setup Action は runner に合う release binary をインストールし、`cff-test` を `PATH` に追加します。Linux と macOS の x64・arm64、および Linux x86 runner に対応し、AWS 認証情報は不要です。

`.github/workflows/cloudfront-functions.yml`:

```yaml
name: Test CloudFront Functions

on:
  push:
  pull_request:

permissions:
  contents: read

jobs:
  test:
    runs-on: ubuntu-24.04
    steps:
      - name: Checkout
        uses: actions/checkout@v7

      - name: Setup cff-test
        uses: kaminchu/cff-test/setup@v1

      - name: Test CloudFront Function
        run: |
          cff-test test \
            cloudfront/function.js \
            --event cloudfront/event.json \
            --expected cloudfront/expected.json
```

Action を `v1` のような major tag で指定すると、その major version の最新 release がインストールされます。CLI の version を個別に固定する場合は、`version` に完全な release tag を指定します。

```yaml
- uses: kaminchu/cff-test/setup@v1
  with:
    version: v1.0.0
```

`cff-test test` は成功時に終了コード `0`、互換性違反や期待値との差異がある場合に `1` を返すため、そのまま GitHub Actions job の成功・失敗として扱えます。

## GitLab CI

以下は Linux amd64 runner で release binary を使い、CloudFront Functions を GitLab CI からテストする例です。`vX.Y.Z` を実際に利用する release tag へ置き換え、`cloudfront/` 以下のファイルパスもリポジトリ構成に合わせて変更してください。

`.gitlab-ci.yml`:

```yaml
variables:
  CFF_TEST_VERSION: "vX.Y.Z"

cloudfront-functions:
  image: ubuntu:24.04
  before_script:
    - apt-get update
    - apt-get install --yes --no-install-recommends ca-certificates curl
    - |
      asset="cff-test-${CFF_TEST_VERSION}-x86_64-unknown-linux-gnu"
      curl --fail --location --silent --show-error \
        --output ./cff-test \
        "https://github.com/kaminchu/cff-test/releases/download/${CFF_TEST_VERSION}/${asset}"
      chmod +x ./cff-test
  script:
    - >-
      ./cff-test test
      cloudfront/function.js
      --event cloudfront/event.json
      --expected cloudfront/expected.json
```

`cff-test test` は成功時に終了コード `0`、互換性違反や期待値との差異がある場合に `1` を返すため、そのまま GitLab CI job の成功・失敗として扱えます。Linux arm64 runner などを使う場合は、[配布ファイル一覧](#github-releases-からダウンロード)に合わせて asset 名を変更してください。

## コマンド

```text
cff-test check <FUNCTION>
cff-test run <FUNCTION> --event <EVENT> [--kvs <KVS>] [--now-ms <MILLISECONDS>]
cff-test test <FUNCTION> --event <EVENT> --expected <EXPECTED> [--kvs <KVS>] [--now-ms <MILLISECONDS>]
cff-test <FUNCTION> --event <EVENT> --expected <EXPECTED> [--kvs <KVS>] [--now-ms <MILLISECONDS>]
```

| コマンド | 動作 |
| --- | --- |
| `check` | 関数コードを静的検査します。event は実行しません。 |
| `run` | event を使って関数を実行し、戻り値 JSON を標準出力へ出します。 |
| `test` | 関数の戻り値と期待値 JSON を比較します。 |
| command 省略 | `test` と同じ動作です。 |

`FUNCTION` は UTF-8 の JavaScript ファイル、`EVENT` と `EXPECTED` は UTF-8 の JSON ファイルです。`EVENT` には CloudFront Functions event version `1.0` を、`EXPECTED` には handler が返す request または response を指定します。

`console.log()` の内容と診断・エラーは標準エラー出力へ出るため、`run` の標準出力は JSON として別のコマンドへ渡せます。

### 終了コード

| code | 意味 |
| --- | --- |
| `0` | 検査、実行、比較に成功 |
| `1` | 互換性、event、実行、戻り値、比較のエラー |
| `2` | CLI の使い方、ファイル I/O、JSON 構文のエラー |

### 時刻を固定する

`--now-ms` に Unix epoch からのミリ秒を指定すると、invocation 中の `Date` がその時刻に固定されます。時刻に依存する関数を再現可能にテストするときに使用します。

```sh
cff-test test function.js --event event.json --expected expected.json --now-ms 0
```

## ローカル KVS fixture

`--kvs` には次の形式の UTF-8 JSON を渡します。`bytes` の value は standard base64 です。

```json
{
  "values": {
    "plain": { "format": "string", "value": "hello" },
    "settings": { "format": "json", "value": { "enabled": true } },
    "binary": { "format": "bytes", "value": "AQID" }
  },
  "meta": {
    "creationDateTime": "2024-01-01T00:00:00.000Z",
    "lastUpdatedDateTime": "2024-01-02T00:00:00.000Z",
    "keyCount": 3
  }
}
```

関数からは `cloudfront` module を介して読み取ります。

```js
import cf from "cloudfront";

const kvs = cf.kvs();

async function handler(event) {
  event.request.headers["x-feature"] = { value: await kvs.get("plain") };
  return event.request;
}
```

```sh
cff-test run function.js --event event.json --kvs kvs.json
```

KVS fixture は read-only です。`get()`、`exists()`、`meta()` に対応します。

## 対応範囲と制約

対象は runtime 2.0 の viewer request / viewer response と event version `1.0` です。対応 module は `crypto`、`querystring`、ローカル fixture の `cloudfront` です。関数コードは UTF-8 byte 長 10 KiB 以下でなければなりません。

QuickJS の実行には local safety limit（64 MiB、約1秒、512 KiB stack）を適用します。ただし、AWS の ComputeUtilization、実 engine の性能・内部挙動、完全一致するエラー文は再現しません。

runtime 1.0、Connection Functions、Lambda@Edge、`cloudfront.cwt`、ネットワーク、ファイルシステム、npm/local file module は対象外です。組み込み global の allowlist、JavaScript 構文、既知の差異を含む一覧は [docs/compatibility.md](docs/compatibility.md) にまとめています。

## 開発

ローカルで全テストを実行します。

```sh
cargo test --locked
```

release binary を作る場合:

```sh
cargo build --locked --release
```

## ライセンス

[MIT License](LICENSE)

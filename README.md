# cff-test

`cff-test` は、CloudFront Functions JavaScript runtime 2.0 の viewer request / viewer response 関数を、AWS へ接続せずに静的検査・実行・JSON 比較する Rust CLI です。QuickJS と対応する組み込み module は実行ファイルへ内包されます。

## Build

```sh
cargo build --release
```

Linux x86_64 で検証しています。`cff-test` は信頼できるコードの互換性テストを目的とした capability 制限であり、信頼できない JavaScript 用のセキュリティ sandbox ではありません。

## Usage

```text
cff-test check <FUNCTION>
cff-test run <FUNCTION> -i <EVENT> [--kvs <KVS>] [--now-ms <MILLISECONDS>]
cff-test test <FUNCTION> -i <EVENT> -o <EXPECTED> [--kvs <KVS>] [--now-ms <MILLISECONDS>]
cff-test <FUNCTION> -i <EVENT> -o <EXPECTED> [--kvs <KVS>] [--now-ms <MILLISECONDS>]
```

`check` は互換性診断を標準エラー出力へ出します。`run` は戻り値 JSON を標準出力へ出し、`test` は JSON Pointer 単位で比較します。`console.log()` は標準エラー出力へ出ます。

```sh
cargo run -- check src/function.js
cargo run -- run src/function.js -i event.json --now-ms 0
cargo run -- test src/function.js -i event.json -o expected.json
```

終了コードは、成功が `0`、関数の互換性・イベント・実行・比較エラーが `1`、CLI・ファイル I/O・JSON 構文エラーが `2` です。

## Local KVS fixture

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

```js
import cf from "cloudfront";
const kvs = cf.kvs();

async function handler(event) {
  event.request.headers["x-feature"] = { value: await kvs.get("plain") };
  return event.request;
}
```

## Scope and restrictions

対象は runtime 2.0 の viewer request / viewer response と event version `1.0` です。対応 module は `crypto`、`querystring`、ローカル fixture の `cloudfront` です。関数コードは UTF-8 byte 長 10 KiB 以下でなければなりません。

`Date` の現在時刻は invocation 中に固定され、`--now-ms` で指定できます。KVS は read-only fixture です。AWS の ComputeUtilization、実エンジンの性能・内部挙動、完全一致するエラー文、runtime 1.0、Connection Functions、`cloudfront.cwt`、ネットワーク、ファイルシステムは再現しません。

組み込み global の allowlist と既知の差異は [docs/compatibility.md](docs/compatibility.md) にまとめています。

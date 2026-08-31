# AWS CloudFront Functions examples

このディレクトリには、AWS Developer Guide の [CloudFront Functions examples for CloudFront](https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/service_code_examples_cloudfront_functions_examples.html) に掲載されている JavaScript runtime 2.0 の全 12 例を収録しています。

JavaScript ファイルは AWS 掲載コードをそのまま保存しています。`suite.json` は各関数に対する入力イベント、期待値、必要なローカル KVS fixture をまとめたテストスイートです。

## テストの実行

リポジトリのルートで次のコマンドを実行します。

```sh
cargo run --locked -- test --suite examples/suite.json
```

ビルド済みの実行ファイルを使う場合は次のように実行できます。

```sh
target/debug/cff-test test --suite examples/suite.json
```

成功時は各 case に `PASS` が表示され、最後に次の集計が表示されます。

```text
RESULT: 12 passed, 0 failed, 0 skipped
```

個別の JavaScript の runtime 2.0 互換性だけを確認する場合は、例えば次を実行します。

```sh
cargo run --locked -- check examples/add-security-headers.js
```

イベントを指定して単独実行する場合は、`suite.json` 内の対象 case の `event` を JSON ファイルに保存し、次の形式で実行します。KVS を利用する例では同様に case の `kvs` を別の JSON ファイルに保存して `--kvs` で指定します。

```sh
cargo run --locked -- run examples/add-security-headers.js --event event.json
cargo run --locked -- run examples/kvs-key-value-pairs.js --event event.json --kvs kvs.json
```

`select-origin-based-on-country.js` のテストは `cloudfront.updateRequestOrigin()` を呼び出して関数が正常終了することと、返却される request を検証します。ローカルテストでは実際の CloudFront origin は変更されないため、origin 選択の最終確認は AWS 上で行ってください。

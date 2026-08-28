# Compatibility

`cff-test` の初期版は CloudFront Functions JavaScript runtime 2.0 の viewer request / viewer response を対象にします。event は version `1.0` だけを受け付けます。

## Supported

- ES 5.1 の基本構文と、runtime 2.0 が追加する `const`、`let`、template literal、arrow function、rest parameter、`async` / `await`、exponentiation operator。
- runtime 2.0 の Object、String、Number、Math、Date、RegExp、JSON、Array、TypedArray、ArrayBuffer、Promise、DataView、Symbol、TextEncoder、TextDecoder の allowlist member。
- global `Buffer` と runtime 2.0 の static / prototype member。
- `require()` または ES module import の `crypto`、`querystring`。
- `import cf from "cloudfront"` の `kvs()`、handle の `get()` / `exists()` / `meta()`。KVS の値はローカル fixture から読み込みます。
- viewer request / viewer response の入力・戻り値構造、lowercase header、status code、body encoding、multiValue の検証。

## Partial or local-only

- QuickJS は単一バイナリへ静的に組み込みますが、AWS と同じ性能や memory quota は保証しません。無限ループ・メモリ過大使用には local safety limit（64 MiB、約1秒、512 KiB stack）を適用します。
- `Buffer.allocUnsafe()` は過去のメモリ内容を公開せず、ゼロ初期化になる場合があります。
- KVS の Promise はネットワークではなく、QuickJS の job queue で同期的に settle します。
- `async` は function declaration と handler の実行に限ります。async arrow、async function expression、async method は CFF011 で拒否します。
- CloudFront が HTTP へ戻す際の header title-case、multiValue wire rule、cookie wire format はシミュレートせず、handler の戻り JSON を比較します。

## Unsupported

- `eval()`、`Function` constructor、timers、process、environment variables、file system、network API。
- runtime 1.0、Connection Functions、Lambda@Edge、TypeScript、JSX、bundler、npm/local file module。
- `cloudfront.cwt`。checker は CFF012、runtime では未公開です。
- ComputeUtilization、AWS 内部 engine bug/quirk、undocumented behavior、AWS と完全一致する error message/stack。

静的検査は、文字列で動的に作った property name や引数経由の capability を推測しません。`obj.fetch` のようなユーザー object property は許可されます。一方、`globalThis["fetch"]`、直接の禁止 global、静的な module 名は検査します。静的検査を通過しても、実行時の capability 制限と戻り値検証が適用されます。

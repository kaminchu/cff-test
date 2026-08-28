# cff-test

Test AWS CloudFront Functions locally and in CI without deploying CloudFront resources or configuring AWS credentials.

`cff-test` is an offline test runner for viewer request and viewer response functions using the CloudFront Functions JavaScript runtime 2.0.

- No AWS account or credentials required
- No CloudFront Function resource required
- Works in GitHub Actions and other CI environments
- Validates CloudFront Functions runtime compatibility
- Supports CloudFront KeyValueStore fixtures

QuickJS and the supported built-in modules are embedded in the executable, so Node.js is not required at runtime.

> [!WARNING]
> The capability restrictions in `cff-test` are intended for compatibility testing of trusted function code. They are not a security sandbox for safely executing untrusted JavaScript.

## Motivation

Even short CloudFront Functions directly affect viewer requests and viewer responses during content delivery. However, deploying every change to AWS for testing slows down feedback and requires both development environments and CI systems to have AWS access.

`cff-test` was created so that function code, input events, and expected results can be kept together in a repository and tested locally or in CI just like regular application code. It aims to detect compatibility violations and unintended behavior at the pull request stage, shortening the feedback loop before deployment to AWS.

It does not replace final verification on AWS. Instead, it provides a fast and reproducible test layer for repeatedly validating day-to-day changes.

## Comparison

| | `cff-test` | AWS `test-function` | Node.js unit tests |
| --- | --- | --- | --- |
| AWS account and credentials required | No | Yes | No |
| CloudFront Function resource required | No | Yes | No |
| Works offline | Yes | No | Yes |
| Runtime compatibility checks | Yes | Runs on the native runtime | No |
| KeyValueStore testing | Local fixture | Associated KVS | Mock required |
| GitHub Actions support | Yes | Yes | Yes |

## Why cff-test instead of `aws cloudfront test-function`?

AWS [`test-function`](https://docs.aws.amazon.com/cloudfront/latest/APIReference/API_TestFunction.html) runs an existing CloudFront Function on AWS. It is useful for verification on the native runtime, but it requires a CloudFront Function resource, AWS credentials, and network access.

`cff-test` is intended for the earlier stage, before creating or updating AWS resources:

```text
pull request -> cff-test -> merge -> update AWS function -> AWS test-function -> publish
```

Use `cff-test` for fast, repeatable local and CI feedback, then use AWS testing when you need to verify behavior on the native runtime.

## Features

- Diagnoses syntax, globals, modules, and members unavailable in CloudFront Functions runtime 2.0 before execution
- Validates viewer request and viewer response events and handler return values
- Prints function return values as formatted JSON
- Displays differences from expected values by JSON Pointer
- Supports `crypto`, `querystring`, and the `cloudfront` module backed by a local KVS fixture
- Reproducibly freezes `Date` with `--now-ms`
- Requires no AWS credentials, network connection, or Node.js

See the [compatibility documentation](docs/compatibility.md) for details about the supported scope.

## Installation

### Download from GitHub Releases

[GitHub Releases](https://github.com/kaminchu/cff-test/releases) provides the following executables for each tag. For example, `<TAG>` in the filenames may be `v0.1.0`.

| Platform | Release asset |
| --- | --- |
| Linux x86 (32-bit) | `cff-test-<TAG>-i686-unknown-linux-gnu` |
| Linux amd64 | `cff-test-<TAG>-x86_64-unknown-linux-gnu` |
| Linux arm64 | `cff-test-<TAG>-aarch64-unknown-linux-gnu` |
| macOS Intel | `cff-test-<TAG>-x86_64-apple-darwin` |
| macOS Apple Silicon | `cff-test-<TAG>-aarch64-apple-darwin` |

Make the downloaded file executable if necessary and place it in a directory on your `PATH`. For example, on Linux amd64:

```sh
chmod +x cff-test-v0.1.0-x86_64-unknown-linux-gnu
mkdir -p "$HOME/.local/bin"
install -m 0755 cff-test-v0.1.0-x86_64-unknown-linux-gnu "$HOME/.local/bin/cff-test"
```

Each release asset is a single executable, not an archive. The macOS binaries are not code-signed or notarized.

### Build from source

You need a Rust toolchain and the C/C++ build toolchain for your target platform. To use Rust 1.96.0, the same version used for release builds:

```sh
git clone https://github.com/kaminchu/cff-test.git
cd cff-test
rustup toolchain install 1.96.0 --profile minimal
cargo +1.96.0 build --locked --release
```

The generated executable is `target/release/cff-test`.

## Quick start

This example uses a viewer request function that rewrites a URI to demonstrate static checking, execution, and comparison against an expected value.

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
# Statically check compatibility with runtime 2.0
cff-test check function.js

# Run the function and print its return value to standard output
cff-test run function.js --event event.json

# Compare the return value with expected.json
cff-test test function.js --event event.json --expected expected.json
```

On success, `check` prints `OK: function.js is compatible with cloudfront-js-2.0`, and `test` prints `PASS: function.js`.

## GitHub Actions

Test CloudFront Functions in GitHub Actions without AWS credentials. This example uses a release binary on a Linux amd64 runner. Replace `vX.Y.Z` with the release tag you want to use, and adjust the file paths under `cloudfront/` to match your repository layout. Pinning the version ensures that the same `cff-test` version is used for CI runs on the same revision.

`.github/workflows/cloudfront-functions.yml`:

```yaml
name: Test CloudFront Functions

on:
  push:
  pull_request:

permissions:
  contents: read

env:
  CFF_TEST_VERSION: vX.Y.Z

jobs:
  test:
    runs-on: ubuntu-24.04
    steps:
      - name: Checkout
        uses: actions/checkout@v7

      - name: Install cff-test
        shell: bash
        run: |
          asset="cff-test-${CFF_TEST_VERSION}-x86_64-unknown-linux-gnu"
          curl --fail --location --silent --show-error \
            --output "${RUNNER_TEMP}/cff-test" \
            "https://github.com/kaminchu/cff-test/releases/download/${CFF_TEST_VERSION}/${asset}"
          chmod +x "${RUNNER_TEMP}/cff-test"

      - name: Test CloudFront Function
        shell: bash
        run: |
          "${RUNNER_TEMP}/cff-test" test \
            cloudfront/function.js \
            --event cloudfront/event.json \
            --expected cloudfront/expected.json
```

`cff-test test` exits with code `0` on success and `1` when it detects a compatibility violation or a difference from the expected value, so the result can be used directly as the GitHub Actions job status. When using a Linux arm64 runner or another platform, change the asset name to match the [list of release assets](#download-from-github-releases).

## GitLab CI

The following example tests CloudFront Functions in GitLab CI using a release binary on a Linux amd64 runner. Replace `vX.Y.Z` with the release tag you want to use, and adjust the file paths under `cloudfront/` to match your repository layout.

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

`cff-test test` exits with code `0` on success and `1` when it detects a compatibility violation or a difference from the expected value, so the result can be used directly as the GitLab CI job status. When using a Linux arm64 runner or another platform, change the asset name to match the [list of release assets](#download-from-github-releases).

## Commands

```text
cff-test check <FUNCTION>
cff-test run <FUNCTION> --event <EVENT> [--kvs <KVS>] [--now-ms <MILLISECONDS>]
cff-test test <FUNCTION> --event <EVENT> --expected <EXPECTED> [--kvs <KVS>] [--now-ms <MILLISECONDS>]
cff-test <FUNCTION> --event <EVENT> --expected <EXPECTED> [--kvs <KVS>] [--now-ms <MILLISECONDS>]
```

| Command | Behavior |
| --- | --- |
| `check` | Statically checks the function code without executing an event. |
| `run` | Runs the function with an event and prints the returned JSON to standard output. |
| `test` | Compares the function's return value with the expected JSON. |
| Command omitted | Behaves the same as `test`. |

`FUNCTION` is a UTF-8 JavaScript file, while `EVENT` and `EXPECTED` are UTF-8 JSON files. Specify CloudFront Functions event version `1.0` in `EVENT`, and the request or response returned by the handler in `EXPECTED`.

Because `console.log()` output, diagnostics, and errors are written to standard error, the standard output from `run` can be piped to another command as JSON.

### Exit codes

| Code | Meaning |
| --- | --- |
| `0` | The check, execution, or comparison succeeded |
| `1` | Compatibility, event, execution, return value, or comparison error |
| `2` | CLI usage, file I/O, or JSON syntax error |

### Freeze time

When `--now-ms` is given a time in milliseconds since the Unix epoch, `Date` is frozen at that time for the duration of the invocation. Use this option to reproducibly test time-dependent functions.

```sh
cff-test test function.js --event event.json --expected expected.json --now-ms 0
```

## Local KVS fixture

Pass `--kvs` a UTF-8 JSON file in the following format. The value of `bytes` must use standard Base64 encoding.

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

Read the fixture from the function through the `cloudfront` module.

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

KVS fixtures are read-only. The `get()`, `exists()`, and `meta()` methods are supported.

## Supported scope and limitations

`cff-test` targets viewer request and viewer response functions on runtime 2.0 using event version `1.0`. Supported modules are `crypto`, `querystring`, and `cloudfront` backed by a local fixture. Function code must be no larger than 10 KiB in UTF-8 bytes.

QuickJS execution is subject to local safety limits: 64 MiB of memory, approximately one second of execution time, and a 512 KiB stack. However, `cff-test` does not reproduce AWS ComputeUtilization, the performance or internal behavior of the production engine, or identical error messages.

Runtime 1.0, Connection Functions, Lambda@Edge, `cloudfront.cwt`, networking, file system access, and npm or local file modules are outside the supported scope. See [docs/compatibility.md](docs/compatibility.md) for the complete list, including the built-in global allowlist, JavaScript syntax support, and known differences.

## Development

Run the full test suite locally:

```sh
cargo test --locked
```

To build a release binary:

```sh
cargo build --locked --release
```

## License

[MIT License](LICENSE)

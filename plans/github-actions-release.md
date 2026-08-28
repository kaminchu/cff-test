# GitHub Actions バイナリ配布 実装計画

## 1. 目的

Git tag が push されたときに GitHub Actions で `cff-test` の release build を行い、5種類の実行ファイルを同じ tag の GitHub Release に添付する。

この計画で変更するのは release workflow の追加だけとする。Rustのアプリケーションコード、`Cargo.toml`、既存テスト、通常のbranch/PR用CI、インストーラー、コード署名、archive、checksum生成は変更・追加しない。

## 2. 要件の解釈と前提

- 「linuxのx86」は32-bit x86を意味するものとし、Rust targetは `i686-unknown-linux-gnu` とする。
- 「linuxのamd64」は64-bit x86を意味するものとし、Rust targetは `x86_64-unknown-linux-gnu` とする。
- 「linuxのarm64」は64-bit ARMを意味するものとし、Rust targetは `aarch64-unknown-linux-gnu` とする。
- 「macOS(Intel)」は `x86_64-apple-darwin` とする。
- 「macOS(Apple Silicon)」は `aarch64-apple-darwin` とする。
- 「バイナリを配布」は、Actionsの一時的なworkflow artifactだけでなく、tagに対応するGitHub Releaseを作成してrelease assetとして公開することを意味する。
- release対象はファイル名にそのまま使用できる tag とする。使用可能文字は英数字、`.`、`_`、`-` で、先頭は英数字とする（例: `v0.1.0`、`0.1.0-rc.1`）。`/` や空白を含む tag は、ファイル名にtag名をそのまま含められないためworkflowを明示的に失敗させる。tag名を暗黙に書き換えない。
- tagと `Cargo.toml` の `package.version` が一致することは今回の要件に含めない。`v0.1.0` tagでも実行時の `--version` はCargo側の `0.1.0` を表示する。
- 配布物はarchiveではなく、rename済みの単一実行ファイルとする。ダウンロード後の実行権限は配布形式では保持されないため、利用者が必要に応じて `chmod +x` を行う。
- Rust toolchainは、この計画作成時の開発環境と同じ `1.96.0` に固定する。releaseの再実行時に `stable` の指す版が変わって成果物が変化することを避ける。
- GitHub.com上のstandard GitHub-hosted runnerを使う。GitHub Enterprise Serverやself-hosted runnerへの対応は対象外とする。

## 3. 生成物の仕様

tagが `v0.1.0` の場合、GitHub Releaseに次の5ファイルが存在する状態を完成形とする。

| 要件上の環境 | runner | Rust target | release asset名 |
| --- | --- | --- | --- |
| Linux x86 | `ubuntu-24.04` | `i686-unknown-linux-gnu` | `cff-test-v0.1.0-linux-x86` |
| Linux amd64 | `ubuntu-24.04` | `x86_64-unknown-linux-gnu` | `cff-test-v0.1.0-linux-amd64` |
| Linux arm64 | `ubuntu-24.04-arm` | `aarch64-unknown-linux-gnu` | `cff-test-v0.1.0-linux-arm64` |
| macOS Intel | `macos-15-intel` | `x86_64-apple-darwin` | `cff-test-v0.1.0-macos-intel` |
| macOS Apple Silicon | `macos-15` | `aarch64-apple-darwin` | `cff-test-v0.1.0-macos-apple-silicon` |

一般形は `cff-test-${TAG}-${PLATFORM}` とし、`${TAG}` には `github.ref_name` を一切加工せず使用する。`${PLATFORM}` は表のasset名末尾にある `linux-x86` などの固定値とする。

## 4. 変更対象

### 4.1 新規作成: `.github/workflows/release.yml`

workflowは1ファイルにまとめ、`build` と `release` の2 jobで構成する。他のファイルは変更しない。

workflow名は `Release binaries` とする。

### 4.2 triggerと権限

- `on.push.tags: ['**']` を指定し、階層を含むものも含めてすべてのtag pushを検知する。対応不能なtag名は後述のvalidationでfailさせる。
- workflow全体のdefault permissionは `contents: read` とする。
- `build` jobにはwrite権限を与えない。
- `release` jobだけ `permissions.contents: write` を指定し、標準の `GITHUB_TOKEN` でGitHub Releaseを作成できるようにする。PATやrepository secretは追加しない。
- 手動実行の `workflow_dispatch` は追加しない。要件どおりtag pushのみを契機とする。

## 5. `build` jobの詳細

### 5.1 matrix

`strategy.fail-fast: false` とし、1 targetの失敗時にも他targetの結果を確認できるようにする。`matrix.include` の各要素は、少なくとも次の4フィールドを持たせる。

```yaml
strategy:
  fail-fast: false
  matrix:
    include:
      - runner: ubuntu-24.04
        target: i686-unknown-linux-gnu
        platform: linux-x86
        install_multilib: true
      - runner: ubuntu-24.04
        target: x86_64-unknown-linux-gnu
        platform: linux-amd64
        install_multilib: false
      - runner: ubuntu-24.04-arm
        target: aarch64-unknown-linux-gnu
        platform: linux-arm64
        install_multilib: false
      - runner: macos-15-intel
        target: x86_64-apple-darwin
        platform: macos-intel
        install_multilib: false
      - runner: macos-15
        target: aarch64-apple-darwin
        platform: macos-apple-silicon
        install_multilib: false
runs-on: ${{ matrix.runner }}
```

macOSの2行を含め、すべてのstepは明示的に `shell: bash` を使える内容に統一する。

### 5.2 実行順序

各matrix jobで次の順に処理する。

1. `actions/checkout@v7` でtagが指すcommitをcheckoutする。`ref` や `fetch-depth` は指定せず、push eventに対するdefault checkoutを使う。
2. tag名を検証する。
   - `TAG` はstepの `env` で `${{ github.ref_name }}` を渡す。expressionをshell script本文へ直接埋め込まない。
   - bashの正規表現 `^[A-Za-z0-9][A-Za-z0-9._-]*$` に一致しなければ、許可文字をstderrへ表示してexit 1する。
   - これにより、tag名をshell commandやパスとして扱う前に不正文字を拒否する。
3. `matrix.install_multilib == true` の場合だけ `sudo apt-get update` と `sudo apt-get install --yes gcc-multilib` を実行する。これは32-bit LinuxのC/C++ linkerとglibc開発ファイルを用意するためであり、他の4 targetでは実行しない。
4. Rust toolchainを次のコマンドで準備する。第三者のtoolchain setup actionは追加しない。

   ```bash
   rustup toolchain install 1.96.0 --profile minimal --no-self-update
   rustup target add --toolchain 1.96.0 "${TARGET}"
   ```

   `TARGET` はstepの `env` から `matrix.target` を渡す。
5. 対象target向けテストを実行する。

   ```bash
   cargo +1.96.0 test --locked --release --target "${TARGET}"
   ```

   `--locked` により、workflow内で `Cargo.lock` を更新せず、lock fileと解決結果が食い違う場合は失敗させる。各runnerは生成したバイナリと同じCPU architectureなので実行を伴うtestが可能である。例外はLinux x86だけだが、`ubuntu-24.04` x64 runner上で `gcc-multilib` により32-bit executableを実行する。
6. 同じtargetをrelease buildする。テストがrelease profileで同じtargetをbuild済みでも、配布対象を明示するため次を実行する。

   ```bash
   cargo +1.96.0 build --locked --release --target "${TARGET}" --bin cff-test
   ```
7. `dist` directoryを作り、`target/${TARGET}/release/cff-test` を `dist/cff-test-${TAG}-${PLATFORM}` へcopyする。sourceをmoveせず、想定したbinaryが存在しなければstepを失敗させる。`TAG` と `PLATFORM` はstepの `env` で渡し、すべて二重引用符で囲む。
8. rename後の配布binaryを、その生成元runner上でsmoke testする。

   ```bash
   "dist/cff-test-${TAG}-${PLATFORM}" --version
   ```

   stdoutに対する脆い完全一致判定は追加せず、exit code 0だけを確認する。機能テストは直前の `cargo test` が担う。
9. `actions/upload-artifact@v7` で、rename済みbinaryをjob間artifactとしてuploadする。
   - artifact名: `release-${{ matrix.platform }}`
   - path: `dist/cff-test-${{ github.ref_name }}-${{ matrix.platform }}`
   - `if-no-files-found: error`
   - 各matrix jobが異なるartifact名を持つようにし、並列uploadの衝突を避ける。
   - ここで作るActions artifactはjob間受け渡し用であり、最終的な配布先ではない。

## 6. `release` jobの詳細

### 6.1 job設定

- `needs: build` とし、5つすべてのmatrix build/test/uploadが成功したときだけ開始する。
- `runs-on: ubuntu-24.04` とする。
- job levelで `permissions: contents: write` を付与する。
- source codeは使わないためcheckout stepは置かない。

この依存関係により、1 targetでも失敗した場合は不完全なGitHub Releaseを作成しない。

### 6.2 artifactの集約

`actions/download-artifact@v8` を次の設定で1回実行する。

```yaml
with:
  pattern: release-*
  path: dist
  merge-multiple: true
```

続くbash stepで、`dist` 直下に期待する5ファイルが各1個だけ存在することを確認する。

- 期待するファイル名をtagから明示的に組み立て、各ファイルに `test -f` を行う。
- `find dist -maxdepth 1 -type f | wc -l` が `5` であることも確認し、過不足があればrelease作成前に失敗させる。
- `ls -l dist` をログへ出し、最終asset名を確認可能にする。

### 6.3 GitHub Releaseの作成

runner imageに含まれるGitHub CLIを使い、次の形でreleaseを作成し、5ファイルを同時に添付する。

```bash
gh release create "${TAG}" dist/* \
  --repo "${GITHUB_REPOSITORY}" \
  --verify-tag \
  --title "${TAG}" \
  --generate-notes
```

stepには次の環境変数を設定する。

```yaml
env:
  GH_TOKEN: ${{ github.token }}
  TAG: ${{ github.ref_name }}
```

- `--verify-tag` により、workflowがtag push以外の参照に誤ってreleaseを作らないようにする。
- release titleはtag名と同じにする。
- release notesはGitHubの自動生成を使う。
- prerelease/draft判定は追加せず、通常のpublished releaseとして作成する。
- 同一tagのreleaseが既に存在する場合は上書きやasset削除をせず、jobを失敗させる。再実行時の自動上書きは今回の要件外とする。
- wildcardを使うのは、直前に `dist` が期待する5ファイルだけであることを検証した後に限定する。

## 7. 実装時に作成するworkflowの骨格

実装者は次の階層を崩さず、上記のstepを埋める。

```yaml
name: Release binaries

on:
  push:
    tags:
      - '**'

permissions:
  contents: read

env:
  RUST_VERSION: 1.96.0

jobs:
  build:
    name: Build ${{ matrix.platform }}
    strategy: # 5 targetのinclude matrix
    runs-on: ${{ matrix.runner }}
    steps:
      - name: Checkout
      - name: Validate tag name
      - name: Install 32-bit Linux build dependencies
        if: matrix.install_multilib
      - name: Install Rust toolchain and target
      - name: Test
      - name: Build release binary
      - name: Prepare release asset
      - name: Smoke test release asset
      - name: Upload release asset

  release:
    name: Publish GitHub Release
    needs: build
    runs-on: ubuntu-24.04
    permissions:
      contents: write
    steps:
      - name: Download release assets
      - name: Verify release assets
      - name: Create GitHub Release
```

`env.RUST_VERSION` を定義する場合、shell command内でversion文字列を重複記述せず `cargo +"${RUST_VERSION}"`、`rustup ... "${RUST_VERSION}"` の形で参照してもよい。ただし、すべてのtoolchain操作とcargo実行が必ず同じ固定versionを使うようにする。

## 8. エラー時の期待動作

| 失敗内容 | 期待動作 |
| --- | --- |
| tag名に `/`、空白、shell metacharacterなどが含まれる | buildのtag validationで失敗し、releaseを作らない |
| `Cargo.lock` が現在のmanifestと不整合 | `cargo --locked` が失敗し、releaseを作らない |
| 1 targetだけbuild/testに失敗 | 他matrix jobは継続して診断可能にし、release jobはskipする |
| binaryのcopyまたはartifact uploadに失敗 | 該当build jobを失敗させ、releaseを作らない |
| 集約したassetが5個でない | release jobの検証で失敗し、releaseを作らない |
| GitHub Release作成権限がない | `gh release create` が失敗し、workflowを成功扱いにしない |
| 同一tagのreleaseが既にある | 既存releaseを変更せず失敗する |

## 9. 検証手順

### 9.1 実装直後のローカル確認

workflow追加前からあるRustコードを壊していないことを、repository rootで次により確認する。

```bash
cargo test --locked
```

さらに `.github/workflows/release.yml` を目視または利用可能なYAML/actionlint系validatorで検査し、次を確認する。

- YAMLとしてparse可能である。
- triggerがtag pushだけである。
- matrixが上記5行と完全に一致する。
- write permissionがrelease jobだけにある。
- build/testの両方に `--locked`、`--release`、`--target` がある。
- release jobが `needs: build` を持つ。

### 9.2 GitHub上の結合確認

安全な検証用tag（例: `v0.1.0-test.1`）を対象commitへpushし、次を確認する。この操作はrelease workflow実装とは別に、repository管理者が明示的に実施する。

1. workflowが1回起動する。
2. `build` matrixが5 job生成され、すべて成功する。
3. Linux x86 jobだけにmultilib install stepが実行され、他ではskipされる。
4. 各jobでtest、release build、`--version` smoke testが成功する。
5. 5 build jobの成功後にだけrelease jobが開始する。
6. tagと同名のpublished GitHub Releaseが作成される。
7. Release Assetsが「3. 生成物の仕様」に記載した5ファイルだけである。
8. 各assetのファイル名にtag名が無加工で含まれる。
9. 少なくとも各対応OS/architectureでassetをdownloadし、必要なら `chmod +x` 後、`./<asset-name> --version` がexit code 0になる。

### 9.3 受け入れ条件

以下をすべて満たした時点で実装完了とする。

- tag push以外ではrelease workflowが起動しない。
- tag pushから5 targetのbuildとtestが行われる。
- 5 targetのうち1つでも失敗した場合、GitHub Releaseは作成されない。
- 全target成功時、tagと同名のGitHub Releaseが作成される。
- Releaseに5つのraw binaryが添付され、名前が規定どおりでtag名を含む。
- workflowで長期PATや追加secretを必要としない。
- Rust source、manifest、既存テストの変更を伴わない。

## 10. 実装時の参照先

- GitHub-hosted runner labels: <https://docs.github.com/en/actions/reference/runners/github-hosted-runners>
- GitHub Actions workflow permissions: <https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax>
- Rust platform support: <https://doc.rust-lang.org/rustc/platform-support.html>
- GitHub CLI `gh release create`: <https://cli.github.com/manual/gh_release_create>
- `actions/checkout`: <https://github.com/actions/checkout>
- `actions/upload-artifact`: <https://github.com/actions/upload-artifact>
- `actions/download-artifact`: <https://github.com/actions/download-artifact>

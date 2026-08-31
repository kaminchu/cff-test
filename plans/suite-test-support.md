# Suite による複数関数・複数ケース実行の実装計画

## 目的

現在の単発テスト形式を維持したまま、1つのJSONファイルに複数のCloudFront Functionと各関数の複数テストケースを定義し、1回のCLI呼び出しでまとめて実行できるようにする。

追加するコマンド形式は次の1つだけとする。

```sh
cff-test test --suite path/to/cff-test.json
```

既存の次の形式は、引数の意味、標準出力、標準エラー出力、終了コードを変更せず維持する。

```sh
cff-test check function.js
cff-test run function.js --event event.json [--kvs kvs.json] [--now-ms 0]
cff-test test function.js --event event.json --expected expected.json [--kvs kvs.json] [--now-ms 0]
cff-test function.js --event event.json --expected expected.json [--kvs kvs.json] [--now-ms 0]
```

## スコープ外

初回実装には以下を含めない。必要になった時点で別途設計する。

- `check`、`run`のsuite対応
- globによる関数やケースの自動探索
- YAML、TOMLなどJSON以外のsuite形式
- suiteファイルのinclude、継承、変数展開
- function単位またはsuite全体の`kvs`、`now_ms`デフォルト値
- ケースのfilter、tag、並列実行、fail-fast
- JSON Schemaファイルの配布
- suite実行結果のJSON/JUnit形式での出力

## CLI仕様

### 正常な組み合わせ

```text
cff-test test --suite <SUITE>
```

`<SUITE>`はUTF-8のJSONファイルとする。

### 排他的な引数

`--suite`を指定したときは、次の引数を受け付けない。

- functionの位置引数
- `--event`
- `--expected`
- `--kvs`
- `--now-ms`

また、`--suite`は明示的な`test`コマンドでのみ使用できる。以下はすべてusage error（終了コード`2`）にする。

```sh
cff-test test function.js --suite cff-test.json
cff-test test --suite cff-test.json --event event.json
cff-test run --suite cff-test.json
cff-test check --suite cff-test.json
cff-test --suite cff-test.json
```

最後の形式を許可しないのは、既存のcommand省略形式では最初の位置引数がfunctionであり、suite実行だけ例外にするとCLI解釈が分かりにくくなるためである。

### help表示

usageに次の行を追加する。

```text
cff-test test --suite <SUITE>
```

`--suite`の説明は `Run test cases defined in a suite JSON file` とする。既存オプションの説明やコマンド省略形式は変更しない。

## Suite JSON仕様

### 完全な例

```json
{
  "functions": [
    {
      "name": "rewrite",
      "function": "functions/rewrite.js",
      "cases": [
        {
          "name": "eventとexpectedをファイルから読む",
          "event": "events/request.json",
          "expected": "expected/rewrite.json"
        },
        {
          "name": "JSONを直接記述する",
          "event": {
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
          },
          "expected": {
            "method": "GET",
            "uri": "/rewritten",
            "querystring": {},
            "headers": { "host": { "value": "example.com" } },
            "cookies": {}
          }
        }
      ]
    },
    {
      "name": "kvs",
      "function": "functions/kvs.js",
      "cases": [
        {
          "name": "KVSをファイルから読む",
          "event": "events/request.json",
          "expected": "expected/kvs.json",
          "kvs": "kvs/local.json"
        },
        {
          "name": "KVSを直接記述する",
          "event": "events/request.json",
          "expected": "expected/kvs.json",
          "kvs": {
            "values": {
              "setting": { "format": "string", "value": "enabled" }
            },
            "meta": {
              "creationDateTime": "2024-01-01T00:00:00.000Z",
              "lastUpdatedDateTime": "2024-01-02T00:00:00.000Z",
              "keyCount": 1
            }
          },
          "now_ms": 0
        }
      ]
    }
  ]
}
```

### 構造

トップレベル:

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `functions` | array | 必須 | 1件以上の関数定義 |

関数定義:

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `name` | string | 必須 | 出力に使う関数名。同一suite内で一意 |
| `function` | string | 必須 | JavaScriptファイルへのパス |
| `cases` | array | 必須 | 1件以上のケース定義 |

ケース定義:

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `name` | string | 必須 | 出力に使うケース名。同じ関数内で一意 |
| `event` | stringまたは任意のJSON値 | 必須 | 文字列ならJSONファイルへのパス、それ以外ならインラインevent |
| `expected` | stringまたは任意のJSON値 | 必須 | 文字列ならJSONファイルへのパス、それ以外ならインライン期待値 |
| `kvs` | stringまたは任意のJSON値 | 任意 | 文字列ならKVS fixtureファイルへのパス、それ以外ならインラインKVS fixture |
| `now_ms` | signed 64-bit integer | 任意 | そのケースの固定時刻。既存`--now-ms`と同じ意味 |

以下の規則を適用する。

- `kvs`はケースにだけ指定できる。関数定義やトップレベルには置けない。
- `kvs`省略時は、そのケースにKVSを関連付けない。直前ケースのKVSを引き継がない。
- `now_ms`もケースごとに独立し、省略時は既存の単発実行と同様に実行時の現在時刻を使用する。
- JSON文字列は常にファイルパスとして解釈する。インラインのJSON文字列としては解釈しない。
- event、handlerの戻り値、KVS fixtureは既存仕様上JSONオブジェクトであるため、前項による実用上の表現欠落はない。
- `null`をインライン指定した場合、ファイル未指定とは扱わず、そのJSON値を既存の検証へ渡す。eventの`null`はevent validation error、KVSの`null`はKVS fixtureの形式エラーになる。
- 未知のフィールドはtypoを黙って無視しないよう、すべての階層でエラーにする。
- `functions`が空、`cases`が空、`name`が空文字または空白だけの場合はsuite構成エラーにする。
- 関数の`name`重複、および同じ関数内のケース`name`重複はsuite構成エラーにする。異なる関数間ではケース名の重複を許可する。

### パス解決

suite内の相対パスは、プロセスのcurrent working directoryではなく、suiteファイルの親ディレクトリを基準に解決する。対象は次のすべてである。

- `function`
- 文字列形式の`event`
- 文字列形式の`expected`
- 文字列形式の`kvs`

絶対パスはそのまま使用する。パスはcanonicalizeしない。存在しないファイルについて既存の`AppError::Io`が元の解決済みパスを表示でき、symlinkの意味も変えないためである。

suite自身を相対パスで渡した場合は、まずCLI起動時のcurrent working directoryに対してsuiteのパスを解決し、その親を基準ディレクトリにする。

## Rust上のデータモデル

`src/suite.rs`を追加し、suiteのdeserialize、構造検証、パス解決、参照JSONの読み込みを担当させる。実行処理は`src/app.rs`に置く。

概ね次の型にする。フィールド名は実装時にもこの名前を使用し、別名やaliasは追加しない。

```rust
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSuite {
    pub functions: Vec<RawSuiteFunction>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSuiteFunction {
    pub name: String,
    pub function: PathBuf,
    pub cases: Vec<RawSuiteCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSuiteCase {
    pub name: String,
    pub event: JsonSource,
    pub expected: JsonSource,
    pub kvs: Option<JsonSource>,
    pub now_ms: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum JsonSource {
    File(PathBuf),
    Inline(serde_json::Value),
}
```

`JsonSource`では`File` variantを先に定義し、JSON文字列が確実にpathとしてdeserializeされるようにする。

実行層にはraw型を直接渡さず、すべての参照ファイルを読み込んだ次のresolved型へ変換する。

```rust
pub struct Suite {
    pub functions: Vec<SuiteFunction>,
}

pub struct SuiteFunction {
    pub name: String,
    pub function_path: PathBuf,
    pub source: String,
    pub cases: Vec<SuiteCase>,
}

pub struct SuiteCase {
    pub name: String,
    pub event: Value,
    pub expected: Value,
    pub kvs: Option<KvsFixture>,
    pub now_ms: Option<i64>,
}
```

`KvsFixture`を`src/suite.rs`から利用可能にするだけでよく、内部フィールドを公開しない。

## 読み込みと検証の順序

部分的にケースを実行した後でsuite構成エラーが見つかることを避けるため、次の順番を固定する。

1. suiteファイルをUTF-8として読む。
2. `serde_json::from_str::<RawSuite>`でdeserializeする。
3. 空配列、空の名前、名前重複を検証する。
4. suiteの親ディレクトリを基準に全パスを解決する。
5. 全functionファイルをUTF-8として読む。
6. ファイル参照の全event、expected、KVSを読み、JSONとしてparseする。
7. ファイルおよびインラインの全KVSを`KvsFixture`へ変換し、既存のKVS検証を行う。
8. ここまで全部成功してから関数の互換性検査とケース実行を開始する。

1〜7のどこかで失敗した場合、ケースは1件も実行せず終了コード`2`で終了する。eventのCloudFront eventとしての意味検証は既存どおり実行フェーズで行い、不正なeventはそのケースのテスト失敗（終了コード`1`）として扱う。

expectedには新しい事前shape検証を追加しない。既存単発テストと同じく、実際の戻り値とのJSON比較だけを行う。

## KVS fixtureの変更

現在の`KvsFixture::from_path`はファイル読み込み、JSON deserialize、fixture固有検証を1つの関数で行っている。インラインJSONにも同一の検証を適用するため、`src/runtime/mod.rs`で次のように責務を分ける。

```rust
impl KvsFixture {
    pub fn from_path(path: &Path) -> AppResult<Self> {
        // 現在と同じファイル読み込みとJSON parse
        // parse後にfrom_valueを呼ぶ
    }

    pub fn from_value(value: Value, location: PathBuf) -> AppResult<Self> {
        // RawKvsへのdeserializeと、現在from_path内にある全fixture検証
    }
}
```

`location`はエラー表示専用であり、ファイルfixtureでは実ファイルパス、インラインfixtureでは次の形式の仮想パスを渡す。

```text
<suite-path>#functions[<function-index>].cases[<case-index>].kvs
```

例:

```text
tests/cff-test.json#functions[1].cases[0].kvs
```

`RawKvs`へのdeserialize失敗、`meta.keyCount`不一致、不正な`format`、string/bytes値の型違反、不正base64という現在の検証内容と文言は、ファイル入力でもインライン入力でも同じにする。`from_value`由来のエラーは行・列を特定できないため、既存`AppError::Json`の`line`と`column`にはともに`1`を設定する。

既存の`KvsFixture::from_path`テストが示す動作と終了コードは変えない。

## エラー型

`src/error.rs`にsuite構造エラーとsuite実行失敗を追加する。

```rust
#[error("invalid suite {path}: {message}")]
Suite {
    path: PathBuf,
    message: String,
},

#[error("{}", format_suite_failures(.passed, &.failures))]
SuiteFailures {
    passed: usize,
    failures: Vec<SuiteCaseFailure>,
},
```

`SuiteCaseFailure`は少なくとも完全なcase label（`<function name> / <case name>`）と、元の`AppError`を文字列化したmessageを保持する。再帰的な`AppError`所有を避けるため、messageはケース失敗を収集するときに`error.to_string()`で確定してよい。

終了コードは次のとおりにする。

- `AppError::Suite`: `2`
- suiteファイル自体または参照ファイルの`Io`/`Json`: 既存どおり`2`
- `AppError::SuiteFailures`: `1`

読み込みフェーズで複数エラーを集約する必要はない。最初の構成・I/O・JSONエラーを返す。

## 実行処理

### 単発処理の分離

`src/app.rs`にある単発テストの核を、suiteからも呼べるprivate helperへ最小限分離する。

```rust
fn execute_checked(
    checked: &CheckedSource,
    event: &Value,
    kvs: Option<KvsFixture>,
    now_ms: Option<i64>,
) -> AppResult<Value>
```

このhelperは以下だけを行う。

1. `validate_event`
2. `RuntimeRunner::execute`
3. `validate_return`
4. actual JSONを返す

単発`run`は返されたactualをpretty-printし、単発`test`は`assert_json_equal`を呼ぶ。既存のエラーmappingと処理順を変えない。

### Suite処理

`run_suite(suite: Suite) -> AppResult<()>`を追加し、関数定義の順、ケース定義の順に逐次実行する。初回実装では並列化しない。出力順を安定させ、runtimeのresource limitや`console.log()`の順序を予測可能にするためである。

各関数について`check(function_path, source)`は1回だけ実行し、成功した`CheckedSource`を全ケースで再利用する。

互換性検査が失敗した場合は、その関数の全ケースを失敗として記録し、各ケースのmessageに同じcompatibility errorを設定する。その関数のJavaScriptは実行せず、次の関数へ進む。これによりsummaryの総数は常にsuiteに宣言されたケース数と一致する。

互換性検査に成功した場合、各ケースを次の順で実行する。

1. `execute_checked`を呼ぶ。
2. actualとcaseのexpectedを`assert_json_equal`で比較する。
3. 成功なら直ちに標準出力へPASS行を出す。
4. 失敗なら`SuiteCaseFailure`へcase labelとエラー文字列を保存し、次のケースへ進む。

ケースごとに所有する`KvsFixture`を`RuntimeOptions`へ渡す。ケース間でruntime、KVS、event、時刻を共有しない。`KvsFixture`がclone可能な現状を利用してよいが、1ケースにつき1回しか実行しないため、不必要なcloneは追加しない。

全ケース成功時はsummaryを標準出力へ出して`Ok(())`を返す。失敗があれば`AppError::SuiteFailures`を返し、`main`の既存処理によって失敗詳細とsummaryを標準エラー出力へ出す。

## 出力仕様

case labelは常に次の形式とする。

```text
<function name> / <case name>
```

成功ケースは定義順に標準出力へ出す。

```text
PASS: rewrite / normal request
PASS: rewrite / query string
RESULT: 2 passed, 0 failed
```

失敗ケースがある場合、成功ケースのPASS行は標準出力へ出し、失敗一覧とsummaryは標準エラー出力へ出す。

```text
FAIL: rewrite / mismatch
JSON values differ (1 differences)
  /uri
    expected: "/expected"
      actual: "/actual"

FAIL: auth / invalid event
event validation failed:
...

RESULT: 1 passed, 2 failed
```

失敗messageの各行を追加でindentしない。既存のdiagnosticやJSON Pointer差分の整形をそのまま保持するためである。failure同士は空行1行で区切る。末尾に余分な空行は付けず、summary末尾は改行する。

0件ケースは禁止するため、`RESULT: 0 passed, 0 failed`は発生しない。

suite内の関数名・ケース名はユーザー入力をそのまま1行へ表示するため、名前に改行文字（`\n`、`\r`）を含む場合はsuite構成エラーにする。タブなど他の文字への追加制限は設けない。

`console.log()`は現状どおり標準エラー出力へ出る。suite runnerでcaptureやprefix付与は行わない。

## `Cli`の内部表現

現在の全コマンド共通フィールドを持つ`Cli`へ`Option<PathBuf>`をもう1つ足すだけでは、無効な組み合わせを実行層でも扱えてしまう。parse完了後の状態を明確にするため、`src/cli.rs`の公開型を次の形へ変更する。

```rust
pub enum Cli {
    Check { function: PathBuf },
    Run {
        function: PathBuf,
        event: PathBuf,
        kvs: Option<PathBuf>,
        now_ms: Option<i64>,
    },
    Test(TestInput),
}

pub enum TestInput {
    Single {
        function: PathBuf,
        event: PathBuf,
        expected: PathBuf,
        kvs: Option<PathBuf>,
        now_ms: Option<i64>,
    },
    Suite { path: PathBuf },
}
```

`RawCli`には`#[arg(long = "suite", value_name = "SUITE")] suite: Option<PathBuf>`を追加する。既存の独自command省略解釈は維持し、`RawCli`から上記enumへ変換する箇所で全組み合わせを検証する。

既存の`Command` enumは不要になるため、この変更によってのみ未使用になった場合は削除する。それ以外のCLI parser構造をclapのsubcommand deriveへ全面変更しない。

## ファイル別の変更

### `src/cli.rs`

- `RawCli`へ`--suite`を追加する。
- parse後の`Cli`を有効な状態だけ表現するenumへ変更する。
- suite専用形式と排他条件を検証する。
- 既存コマンド、省略形式、help/version、usage errorの挙動を維持する。

### `src/suite.rs`（新規）

- raw/resolved suite型と`JsonSource`を定義する。
- suite JSONのdeserializeと構造検証を行う。
- suite基準のパス解決を行う。
- function sourceと参照JSONをすべて事前読み込みする。
- インライン値をcloneではなく所有権移動でresolved型へ渡す。
- インラインKVSの仮想locationを生成する。

### `src/runtime/mod.rs`

- `KvsFixture::from_value`を追加する。
- 既存`from_path`を共通処理へ委譲する。
- KVS fixtureの検証規則とエラー文言を変えない。

### `src/app.rs`

- 新しい`Cli` enumをmatchする。
- event実行処理を`execute_checked`へ分離する。
- `run_suite`を実装する。
- 関数ごとに互換性検査結果を再利用する。
- case failureを集約し、全ケース処理後に返す。
- 単発コマンドの出力を変えない。

### `src/error.rs`

- `Suite`、`SuiteFailures`、`SuiteCaseFailure`を追加する。
- suite failureの決定的なformat関数を追加する。
- 終了コード`2`と`1`を上記仕様どおり割り当てる。

### `src/main.rs`

- `mod suite;`を追加する。
- `main`のエラー表示・終了処理自体は変更しない。

### `tests/cli.rs`と`tests/fixtures/`

- 下記のCLI統合テストと必要最小限のfixtureを追加する。
- 単発CLIの既存テストは変更せず、回帰テストとして残す。

### `README.md`、`README-ja.md`

- コマンド一覧へsuite形式を追加する。
- suite JSON例、文字列とインラインJSONの判定、相対パス基準、case単位の`kvs`/`now_ms`を説明する。
- GitHub Actions例は既存単発例を維持し、suite利用例を追加する。置換はしない。

## テスト計画

テストは外部から観測できる仕様を`tests/cli.rs`で検証し、複雑な構造検証やパス解決に必要な場合だけ`src/suite.rs`へunit testを追加する。

### 1. 既存動作の回帰

- `check`、`run`、`test`、command省略`test`の既存テストがすべて変更なしで通る。
- 単発`test`成功時の `PASS: <function path>` が変わらない。
- 単発のKVS不正が従来どおり終了コード`2`になる。

### 2. CLI引数

- `cff-test test --suite <file>`を受理する。
- `--suite`とfunction位置引数の併用を終了コード`2`で拒否する。
- `--suite`と`--event`、`--expected`、`--kvs`、`--now-ms`の各併用を終了コード`2`で拒否する。
- `run`、`check`、command省略形式での`--suite`を終了コード`2`で拒否する。
- `--help`にsuite usageとoption説明が出る。

### 3. 複数関数・複数ケース

- 2関数以上、各2ケース以上のsuiteを実行し、全PASS行がJSON定義順に出る。
- summaryが正しいpass/fail件数を表示する。
- 全成功時の終了コードが`0`になる。

### 4. ファイル入力とインライン入力

- eventとexpectedが両方ファイルのケースが成功する。
- eventとexpectedが両方インラインのケースが成功する。
- eventだけファイル、expectedだけインライン、およびその逆の混在ケースが成功する。
- KVSのファイル指定とインライン指定が同じ関数で同じ結果になる。
- `null`のインラインeventがpathとして扱われず、event validation failureになる。

### 5. ケース単位のKVSと時刻

- 同じ関数の2ケースに異なるインラインKVSを与え、それぞれ異なるexpectedで成功する。
- KVSありケースの次にKVSなしケースを置き、KVSが引き継がれないことを確認する。後者がKVS必須関数なら後者だけ失敗することを利用してよい。
- 同じDate利用関数へ異なる`now_ms`を指定し、それぞれのexpectedで成功する。
- `now_ms`が別ケースへ引き継がれないことを確認する。

### 6. 相対パス

- suiteを置いたディレクトリとは異なるcurrent working directoryからCLIを起動し、suite内のfunction/event/expected/kvs相対パスがsuiteの親基準で解決される。
- suite内の絶対パスもそのまま動作する。

### 7. Suite構造エラー

以下が終了コード`2`になり、1件もPASSを出さないことを確認する。

- suite JSONの構文エラー
- 必須フィールド欠落
- 未知フィールド
- 空の`functions`
- 空の`cases`
- 空または改行を含むname
- 関数名重複
- 同一関数内のケース名重複
- 存在しない参照ファイル
- event/expected/KVS参照ファイルのJSON構文エラー
- 不正なインラインKVS（`keyCount`不一致を最低1件）

後半のfunction/caseに不正な参照を置き、前半ケースのPASSも出ないことで事前読み込みを確認する。

### 8. テスト失敗の継続

- 最初のケースをexpected mismatch、次のケースを成功にして、後者のPASSが出ることを確認する。
- event validation failureの後も次ケースが実行される。
- runtime failureの後も次ケースが実行される。
- 1件でも失敗すれば最終終了コードが`1`になる。
- failure label、既存の差分詳細、summary件数を確認する。

### 9. 関数互換性エラー

- 互換性違反の関数に2ケースを定義し、両方が失敗件数に入る。
- その後に定義した正常な関数のケースは実行される。
- `CFFxxx`診断がfailure messageに含まれる。
- summaryのpass+failが宣言された総ケース数と一致する。

## 実装順序

1. `tests/cli.rs`へCLI排他条件と最小のsuite成功テストを追加し、未実装による失敗を確認する。
2. `Cli`をenum化して`--suite`をparseできるようにし、既存CLIテストを通す。
3. `src/suite.rs`へraw型、構造検証、パス解決、事前読み込みを実装する。
4. KVS検証を`KvsFixture::from_value`へ分離し、既存KVSテストとインラインKVSテストを通す。
5. `src/app.rs`の単発実行部分をhelperへ分離し、単発CLIの出力・終了コードが不変であることを確認する。
6. suiteの逐次実行、失敗集約、出力を実装する。
7. 複数関数、複数ケース、KVS/時刻のcase分離、継続実行、相対パス、全エラーケースのテストを追加する。
8. `README.md`と`README-ja.md`を更新する。
9. formatter、unit test、CLI統合テスト、lintを実行する。

## 検証コマンド

```sh
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

必要に応じて実装中は対象テストだけを次のように実行してよいが、完了判定には上記3コマンドをすべて使う。

```sh
cargo test --locked --test cli suite
```

## 完了条件

- `cff-test test --suite <SUITE>`で複数関数・複数ケースを定義順に実行できる。
- event、expected、KVSのそれぞれでファイル参照とインラインJSONを使用できる。
- KVSと`now_ms`がcase単位で完全に分離される。
- suite相対パスの基準がsuiteファイルの親ディレクトリになる。
- suite入力エラーでは実行前に終了コード`2`、ケース失敗では残りを実行して終了コード`1`になる。
- 関数の互換性検査は関数ごとに1回だけ行われる。
- 出力とsummaryが本計画の形式に一致する。
- 既存の単発CLIが後方互換である。
- READMEの日英両方に新しい使い方が記載される。
- `cargo fmt --all -- --check`、`cargo test --locked`、`cargo clippy --locked --all-targets -- -D warnings`がすべて成功する。

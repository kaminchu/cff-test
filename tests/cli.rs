use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::{NamedTempFile, tempdir};

fn cff() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("cff-test")
}

fn fixture_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(path)
}

#[test]
fn version_exits_successfully() {
    cff()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(concat!(
            "cff-test ",
            env!("CARGO_PKG_VERSION")
        )));
}

#[test]
fn legacy_input_and_output_options_are_rejected() {
    for option in ["-i", "--input"] {
        cff()
            .args([
                "run",
                "tests/fixtures/functions/rewrite.js",
                option,
                "tests/fixtures/events/request.json",
            ])
            .assert()
            .code(2)
            .stderr(predicate::str::contains("unexpected argument"));
    }

    for option in ["-o", "--output"] {
        cff()
            .args([
                "test",
                "tests/fixtures/functions/rewrite.js",
                "--event",
                "tests/fixtures/events/request.json",
                option,
                "tests/fixtures/expected/rewrite.json",
            ])
            .assert()
            .code(2)
            .stderr(predicate::str::contains("unexpected argument"));
    }
}

#[test]
fn check_reports_success_and_restricted_global() {
    cff()
        .args(["check", "tests/fixtures/functions/rewrite.js"])
        .assert()
        .success()
        .stdout(predicate::str::contains("OK:"));
    cff()
        .args(["check", "tests/fixtures/functions/uses_fetch.js"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("error[CFF004]").and(predicate::str::contains(":2:3:")));
}

#[test]
fn run_and_test_keep_json_on_stdout() {
    cff()
        .args([
            "run",
            "tests/fixtures/functions/rewrite.js",
            "--event",
            "tests/fixtures/events/request.json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"uri\": \"/rewritten\""));
    cff()
        .args([
            "test",
            "tests/fixtures/functions/rewrite.js",
            "--event",
            "tests/fixtures/events/request.json",
            "--expected",
            "tests/fixtures/expected/rewrite.json",
        ])
        .assert()
        .success()
        .stdout("PASS: tests/fixtures/functions/rewrite.js\n");
    cff()
        .args([
            "tests/fixtures/functions/rewrite.js",
            "--event",
            "tests/fixtures/events/request.json",
            "--expected",
            "tests/fixtures/expected/rewrite.json",
        ])
        .assert()
        .success()
        .stdout("PASS: tests/fixtures/functions/rewrite.js\n");
}

#[test]
fn modules_date_kvs_and_limits_work() {
    cff()
        .args([
            "test",
            "tests/fixtures/functions/crypto_rewrite.js",
            "--event",
            "tests/fixtures/events/request.json",
            "--expected",
            "tests/fixtures/expected/crypto_rewrite.json",
        ])
        .assert()
        .success();
    cff()
        .args([
            "test",
            "tests/fixtures/functions/querystring_rewrite.js",
            "--event",
            "tests/fixtures/events/request.json",
            "--expected",
            "tests/fixtures/expected/querystring_rewrite.json",
        ])
        .assert()
        .success();
    cff()
        .args([
            "test",
            "tests/fixtures/functions/kvs.js",
            "--event",
            "tests/fixtures/events/request.json",
            "--expected",
            "tests/fixtures/expected/kvs.json",
            "--kvs",
            "tests/fixtures/kvs/local.json",
        ])
        .assert()
        .success();
    cff()
        .args([
            "run",
            "tests/fixtures/functions/date.js",
            "--event",
            "tests/fixtures/events/request.json",
            "--now-ms",
            "0",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"x-date\":").and(predicate::str::contains("0:0:1:")));
    cff()
        .args([
            "test",
            "tests/fixtures/functions/response.js",
            "--event",
            "tests/fixtures/events/response.json",
            "--expected",
            "tests/fixtures/expected/response.json",
        ])
        .assert()
        .success();
    cff()
        .args([
            "test",
            "tests/fixtures/functions/buffer_crypto.js",
            "--event",
            "tests/fixtures/events/request.json",
            "--expected",
            "tests/fixtures/expected/buffer_crypto.json",
        ])
        .assert()
        .success();
    cff()
        .args([
            "test",
            "tests/fixtures/functions/text_encoding.js",
            "--event",
            "tests/fixtures/events/request.json",
            "--expected",
            "tests/fixtures/expected/text_encoding.json",
        ])
        .assert()
        .success();
    cff()
        .args([
            "test",
            "tests/fixtures/functions/kvs_all.js",
            "--event",
            "tests/fixtures/events/request.json",
            "--expected",
            "tests/fixtures/expected/kvs_all.json",
            "--kvs",
            "tests/fixtures/kvs/local.json",
        ])
        .assert()
        .success();
    cff()
        .args([
            "run",
            "tests/fixtures/functions/infinite.js",
            "--event",
            "tests/fixtures/events/request.json",
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("local safety limit"));
}

#[test]
fn event_and_return_models_reject_invalid_shapes() {
    for event in [
        "invalid_version.json",
        "invalid_header.json",
        "invalid_body.json",
    ] {
        cff()
            .args([
                "run",
                "tests/fixtures/functions/rewrite.js",
                "--event",
                &format!("tests/fixtures/events/{event}"),
            ])
            .assert()
            .code(1)
            .stderr(predicate::str::contains("event validation failed"));
    }
    cff()
        .args([
            "run",
            "tests/fixtures/functions/invalid_return_header.js",
            "--event",
            "tests/fixtures/events/request.json",
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("/headers/Host"));
    cff()
        .args([
            "run",
            "tests/fixtures/functions/changes_method.js",
            "--event",
            "tests/fixtures/events/request.json",
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("read-only"));
}

#[test]
fn every_static_diagnostic_family_is_reported() {
    for (function, rule) in [
        ("uses_syntax_error.js", "CFF001"),
        ("uses_class.js", "CFF002"),
        ("uses_member.js", "CFF006"),
        ("uses_console.js", "CFF007"),
        ("uses_module.js", "CFF008"),
        ("no_handler.js", "CFF009"),
        ("uses_async_arrow.js", "CFF011"),
    ] {
        cff()
            .args(["check", &format!("tests/fixtures/functions/{function}")])
            .assert()
            .code(1)
            .stderr(predicate::str::contains(format!("error[{rule}]")));
    }
    cff()
        .args(["check", "tests/fixtures/functions/local_names.js"])
        .assert()
        .success();
}

#[test]
fn serialization_and_kvs_fixture_errors_are_classified() {
    for function in ["undefined_return.js", "cyclic_return.js"] {
        cff()
            .args([
                "run",
                &format!("tests/fixtures/functions/{function}"),
                "--event",
                "tests/fixtures/events/request.json",
            ])
            .assert()
            .code(1)
            .stderr(predicate::str::contains("JavaScript serialization failed"));
    }
    cff()
        .args([
            "run",
            "tests/fixtures/functions/kvs.js",
            "--event",
            "tests/fixtures/events/request.json",
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "JavaScript module evaluation failed",
        ));
    cff()
        .args([
            "run",
            "tests/fixtures/functions/kvs.js",
            "--event",
            "tests/fixtures/events/request.json",
            "--kvs",
            "tests/fixtures/kvs/invalid_key_count.json",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("meta.keyCount"));
}

#[test]
fn function_source_uses_a_utf8_byte_limit() {
    let base = b"function handler(event) { return event.request; }\n";
    let mut function = NamedTempFile::new().unwrap();
    let mut exact = base.to_vec();
    exact.resize(10 * 1024, b' ');
    function.write_all(&exact).unwrap();
    function.flush().unwrap();
    cff()
        .args(["check", function.path().to_str().unwrap()])
        .assert()
        .success();

    function.as_file_mut().set_len(0).unwrap();
    function.seek(SeekFrom::Start(0)).unwrap();
    function
        .write_all(&[exact.as_slice(), b"x"].concat())
        .unwrap();
    function.flush().unwrap();
    cff()
        .args(["check", function.path().to_str().unwrap()])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("CFF010"));
}

#[test]
fn assertion_and_unsupported_feature_are_diagnostics() {
    cff()
        .args([
            "test",
            "tests/fixtures/functions/rewrite.js",
            "--event",
            "tests/fixtures/events/request.json",
            "--expected",
            "tests/fixtures/expected/mismatch.json",
        ])
        .assert()
        .code(1)
        .stderr(
            predicate::str::contains("/uri").and(
                predicate::str::contains("expected:").and(predicate::str::contains("actual:")),
            ),
        );
    cff()
        .args(["check", "tests/fixtures/functions/uses_eval.js"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("CFF003"));
    cff()
        .args(["check", "tests/fixtures/functions/uses_timer.js"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("CFF005"));
    cff()
        .args(["check", "tests/fixtures/functions/uses_cwt.js"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("CFF012"));
}

#[test]
fn suite_help_and_argument_exclusivity_are_explicit() {
    cff().arg("--help").assert().success().stdout(
        predicate::str::contains("cff-test test --suite <SUITE>").and(predicate::str::contains(
            "Run test cases defined in a suite JSON file",
        )),
    );

    for args in [
        vec!["test", "function.js", "--suite", "suite.json"],
        vec!["test", "--suite", "suite.json", "--event", "event.json"],
        vec![
            "test",
            "--suite",
            "suite.json",
            "--expected",
            "expected.json",
        ],
        vec!["test", "--suite", "suite.json", "--kvs", "kvs.json"],
        vec!["test", "--suite", "suite.json", "--now-ms", "0"],
        vec!["run", "--suite", "suite.json"],
        vec!["check", "--suite", "suite.json"],
        vec!["suite.json", "--suite", "suite.json"],
        vec!["--suite", "suite.json"],
    ] {
        cff().args(args).assert().code(2);
    }
}

#[test]
fn suite_runs_in_order_from_its_directory() {
    let directory = tempdir().unwrap();
    let suite = fixture_path("suites/basic.json");
    cff()
        .current_dir(directory.path())
        .args(["test", "--suite", suite.to_str().unwrap()])
        .assert()
        .success()
        .stdout(concat!(
            "PASS: rewrite / file inputs\n",
            "PASS: rewrite / inline inputs\n",
            "SKIP: rewrite / disabled case\n",
            "PASS: querystring / file inputs\n",
            "PASS: querystring / explicit false skip\n",
            "RESULT: 4 passed, 0 failed, 1 skipped\n",
        ));
}

#[test]
fn suite_continues_after_case_failures_and_reports_summary() {
    let suite = fixture_path("suites/failure.json");
    cff()
        .args(["test", "--suite", suite.to_str().unwrap()])
        .assert()
        .code(1)
        .stdout("PASS: rewrite / continues\n")
        .stderr(
            predicate::str::contains("FAIL: rewrite / mismatch")
                .and(predicate::str::contains("/uri"))
                .and(predicate::str::contains(
                    "RESULT: 1 passed, 1 failed, 0 skipped",
                )),
        );
}

#[test]
fn suite_cases_keep_kvs_separate() {
    let suite = fixture_path("suites/kvs.json");
    cff()
        .args(["test", "--suite", suite.to_str().unwrap()])
        .assert()
        .code(1)
        .stdout(concat!(
            "PASS: kvs / file fixture\n",
            "PASS: kvs / inline fixture\n",
        ))
        .stderr(
            predicate::str::contains("FAIL: kvs / no fixture")
                .and(predicate::str::contains(
                    "JavaScript module evaluation failed",
                ))
                .and(predicate::str::contains(
                    "RESULT: 2 passed, 1 failed, 0 skipped",
                )),
        );
}

#[test]
fn suite_cases_keep_time_separate() {
    let suite = fixture_path("suites/date.json");
    cff()
        .args(["test", "--suite", suite.to_str().unwrap()])
        .assert()
        .success()
        .stdout(concat!(
            "PASS: date / epoch\n",
            "PASS: date / one second later\n",
            "RESULT: 2 passed, 0 failed, 0 skipped\n",
        ));
}

#[test]
fn suite_skips_compatibility_check_when_all_cases_are_skipped() {
    let suite = fixture_path("suites/all-skipped.json");
    cff()
        .args(["test", "--suite", suite.to_str().unwrap()])
        .assert()
        .success()
        .stdout(concat!(
            "SKIP: unsupported / disabled\n",
            "RESULT: 0 passed, 0 failed, 1 skipped\n",
        ))
        .stderr("");

    let suite = fixture_path("suites/compatibility.json");
    cff()
        .args(["test", "--suite", suite.to_str().unwrap()])
        .assert()
        .code(1)
        .stdout("SKIP: unsupported / disabled\n")
        .stderr(
            predicate::str::contains("FAIL: unsupported / enabled")
                .and(predicate::str::contains("CFF004"))
                .and(predicate::str::contains(
                    "RESULT: 0 passed, 1 failed, 1 skipped",
                )),
        );
}

#[test]
fn suite_validates_null_inline_event_and_all_references_before_running() {
    let null_event = fixture_path("suites/null-event.json");
    cff()
        .args(["test", "--suite", null_event.to_str().unwrap()])
        .assert()
        .code(1)
        .stderr(
            predicate::str::contains("event validation failed")
                .and(predicate::str::contains("I/O error").not()),
        );

    let invalid_later = fixture_path("suites/invalid-later.json");
    cff()
        .args(["test", "--suite", invalid_later.to_str().unwrap()])
        .assert()
        .code(2)
        .stdout("")
        .stderr(predicate::str::contains("I/O error"));

    let null_kvs = fixture_path("suites/null-kvs.json");
    cff()
        .args(["test", "--suite", null_kvs.to_str().unwrap()])
        .assert()
        .code(2)
        .stdout("")
        .stderr(
            predicate::str::contains("#functions[0].cases[0].kvs")
                .and(predicate::str::contains("invalid JSON")),
        );
}

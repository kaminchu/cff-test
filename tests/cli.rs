use std::io::{Seek, SeekFrom, Write};

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::NamedTempFile;

fn cff() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("cff-test")
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
            "-i",
            "tests/fixtures/events/request.json",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"uri\": \"/rewritten\""));
    cff()
        .args([
            "test",
            "tests/fixtures/functions/rewrite.js",
            "-i",
            "tests/fixtures/events/request.json",
            "-o",
            "tests/fixtures/expected/rewrite.json",
        ])
        .assert()
        .success()
        .stdout("PASS: tests/fixtures/functions/rewrite.js\n");
    cff()
        .args([
            "tests/fixtures/functions/rewrite.js",
            "-i",
            "tests/fixtures/events/request.json",
            "-o",
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
            "-i",
            "tests/fixtures/events/request.json",
            "-o",
            "tests/fixtures/expected/crypto_rewrite.json",
        ])
        .assert()
        .success();
    cff()
        .args([
            "test",
            "tests/fixtures/functions/querystring_rewrite.js",
            "-i",
            "tests/fixtures/events/request.json",
            "-o",
            "tests/fixtures/expected/querystring_rewrite.json",
        ])
        .assert()
        .success();
    cff()
        .args([
            "test",
            "tests/fixtures/functions/kvs.js",
            "-i",
            "tests/fixtures/events/request.json",
            "-o",
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
            "-i",
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
            "-i",
            "tests/fixtures/events/response.json",
            "-o",
            "tests/fixtures/expected/response.json",
        ])
        .assert()
        .success();
    cff()
        .args([
            "test",
            "tests/fixtures/functions/buffer_crypto.js",
            "-i",
            "tests/fixtures/events/request.json",
            "-o",
            "tests/fixtures/expected/buffer_crypto.json",
        ])
        .assert()
        .success();
    cff()
        .args([
            "test",
            "tests/fixtures/functions/text_encoding.js",
            "-i",
            "tests/fixtures/events/request.json",
            "-o",
            "tests/fixtures/expected/text_encoding.json",
        ])
        .assert()
        .success();
    cff()
        .args([
            "test",
            "tests/fixtures/functions/kvs_all.js",
            "-i",
            "tests/fixtures/events/request.json",
            "-o",
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
            "-i",
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
                "-i",
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
            "-i",
            "tests/fixtures/events/request.json",
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("/headers/Host"));
    cff()
        .args([
            "run",
            "tests/fixtures/functions/changes_method.js",
            "-i",
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
                "-i",
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
            "-i",
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
            "-i",
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
            "-i",
            "tests/fixtures/events/request.json",
            "-o",
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

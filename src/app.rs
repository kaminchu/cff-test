use std::{fs, path::Path};

use serde_json::Value;

use crate::{
    assertion::assert_json_equal,
    checker::check,
    cli::{Cli, TestInput},
    error::{AppError, AppResult, SuiteCaseFailure},
    event::{validate_event, validate_return},
    runtime::{KvsFixture, RuntimeOptions, RuntimeRunner},
    suite::{Suite, SuiteCase},
};

pub fn run(cli: Cli) -> AppResult<()> {
    match cli {
        Cli::Check { function } => {
            let source = read_utf8(&function)?;
            check(&function, &source)?;
            println!(
                "OK: {} is compatible with cloudfront-js-2.0",
                function.display()
            );
            Ok(())
        }
        Cli::Run {
            function,
            event,
            kvs,
            now_ms,
        } => run_single(function, event, None, kvs, now_ms),
        Cli::Test(TestInput::Single {
            function,
            event,
            expected,
            kvs,
            now_ms,
        }) => run_single(function, event, Some(expected), kvs, now_ms),
        Cli::Test(TestInput::Suite { path }) => run_suite(Suite::from_path(&path)?),
    }
}

fn run_single(
    function: std::path::PathBuf,
    event_path: std::path::PathBuf,
    expected_path: Option<std::path::PathBuf>,
    kvs_path: Option<std::path::PathBuf>,
    now_ms: Option<i64>,
) -> AppResult<()> {
    let source = read_utf8(&function)?;
    let checked = check(&function, &source)?;
    let event = read_json(&event_path)?;
    validate_event(&event).map_err(AppError::EventValidation)?;
    let kvs = kvs_path
        .as_ref()
        .map(|path| KvsFixture::from_path(path))
        .transpose()?;
    let actual = execute_checked(&checked, &event, kvs, now_ms)?;

    if let Some(expected_path) = expected_path {
        let expected = read_json(&expected_path)?;
        assert_json_equal(&expected, &actual).map_err(AppError::Assertion)?;
        println!("PASS: {}", function.display());
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&actual).expect("Value is JSON")
        );
    }
    Ok(())
}

fn execute_checked(
    checked: &crate::checker::CheckedSource,
    event: &Value,
    kvs: Option<KvsFixture>,
    now_ms: Option<i64>,
) -> AppResult<Value> {
    validate_event(event).map_err(AppError::EventValidation)?;
    let actual = RuntimeRunner::execute(checked, event, RuntimeOptions { now_ms, kvs })?;
    validate_return(
        &actual,
        event.get("context").and_then(|c| c.get("eventType")),
        event
            .get("request")
            .and_then(|request| request.get("method"))
            .and_then(Value::as_str),
    )
    .map_err(AppError::ReturnValidation)?;
    Ok(actual)
}

fn run_suite(suite: Suite) -> AppResult<()> {
    let mut passed = 0;
    let mut skipped = 0;
    let mut failures = Vec::new();

    for function in suite.functions {
        let has_runnable_case = function.cases.iter().any(|case| !case.skip);
        let checked = has_runnable_case.then(|| check(&function.function_path, &function.source));
        for case in function.cases {
            let SuiteCase {
                name,
                event,
                expected,
                kvs,
                now_ms,
                skip,
            } = case;
            let label = format!("{} / {name}", function.name);
            if skip {
                skipped += 1;
                println!("SKIP: {label}");
                continue;
            }

            let result = match checked.as_ref() {
                Some(Ok(checked)) => execute_suite_case(checked, &event, &expected, kvs, now_ms),
                Some(Err(error)) => Err(error.to_string()),
                None => unreachable!("a runnable case requires a compatibility check"),
            };
            match result {
                Ok(()) => {
                    passed += 1;
                    println!("PASS: {label}");
                }
                Err(message) => failures.push(SuiteCaseFailure { label, message }),
            }
        }
    }

    if failures.is_empty() {
        println!("RESULT: {passed} passed, 0 failed, {skipped} skipped");
        Ok(())
    } else {
        Err(AppError::SuiteFailures {
            passed,
            skipped,
            failures,
        })
    }
}

fn execute_suite_case(
    checked: &crate::checker::CheckedSource,
    event: &Value,
    expected: &Value,
    kvs: Option<KvsFixture>,
    now_ms: Option<i64>,
) -> Result<(), String> {
    let actual = execute_checked(checked, event, kvs, now_ms).map_err(|error| error.to_string())?;
    assert_json_equal(expected, &actual).map_err(|error| error.to_string())
}

fn read_utf8(path: &Path) -> AppResult<String> {
    let bytes = fs::read(path).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    String::from_utf8(bytes).map_err(|error| AppError::Json {
        path: path.to_path_buf(),
        line: 1,
        column: error.utf8_error().valid_up_to() + 1,
        message: "function source is not valid UTF-8".into(),
    })
}

fn read_json(path: &Path) -> AppResult<Value> {
    let text = read_utf8(path)?;
    serde_json::from_str(&text).map_err(|error| AppError::Json {
        path: path.to_path_buf(),
        line: error.line(),
        column: error.column(),
        message: error.to_string(),
    })
}

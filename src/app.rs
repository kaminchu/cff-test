use std::{fs, path::Path};

use serde_json::Value;

use crate::{
    assertion::assert_json_equal,
    checker::check,
    cli::{Cli, Command},
    error::{AppError, AppResult},
    event::{validate_event, validate_return},
    runtime::{KvsFixture, RuntimeOptions, RuntimeRunner},
};

pub fn run(cli: Cli) -> AppResult<()> {
    let source = read_utf8(&cli.function)?;
    let checked = check(&cli.function, &source)?;

    if cli.command == Command::Check {
        println!(
            "OK: {} is compatible with cloudfront-js-2.0",
            cli.function.display()
        );
        return Ok(());
    }

    let event_path = cli.event.as_ref().expect("CLI validation ensures event");
    let event = read_json(event_path)?;
    validate_event(&event).map_err(AppError::EventValidation)?;

    let kvs = cli
        .kvs
        .as_ref()
        .map(|path| KvsFixture::from_path(path))
        .transpose()?;
    let options = RuntimeOptions {
        now_ms: cli.now_ms,
        kvs,
    };
    let actual = RuntimeRunner::execute(&checked, &event, options)?;
    validate_return(
        &actual,
        event.get("context").and_then(|c| c.get("eventType")),
        event
            .get("request")
            .and_then(|request| request.get("method"))
            .and_then(Value::as_str),
    )
    .map_err(AppError::ReturnValidation)?;

    match cli.command {
        Command::Run => {
            println!(
                "{}",
                serde_json::to_string_pretty(&actual).expect("Value is JSON")
            );
        }
        Command::Test => {
            let expected_path = cli
                .expected
                .as_ref()
                .expect("CLI validation ensures expected value");
            let expected = read_json(expected_path)?;
            assert_json_equal(&expected, &actual).map_err(AppError::Assertion)?;
            println!("PASS: {}", cli.function.display());
        }
        Command::Check => unreachable!(),
    }

    Ok(())
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

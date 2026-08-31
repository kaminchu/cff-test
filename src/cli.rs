use std::path::PathBuf;

use clap::{Parser, error::ErrorKind};

use crate::error::{AppError, AppResult};

#[derive(Debug, Parser)]
#[command(
    name = "cff-test",
    version,
    about = "Test CloudFront Functions locally",
    after_help = "Suite usage:\n  cff-test test --suite <SUITE>"
)]
struct RawCli {
    #[arg(value_name = "COMMAND|FUNCTION")]
    target: String,

    #[arg(value_name = "FUNCTION", requires = "target")]
    function: Option<PathBuf>,

    #[arg(long = "event", value_name = "EVENT")]
    event: Option<PathBuf>,

    #[arg(long = "expected", value_name = "EXPECTED")]
    expected: Option<PathBuf>,

    #[arg(long = "kvs", value_name = "KVS")]
    kvs: Option<PathBuf>,

    #[arg(long = "now-ms", value_name = "MILLISECONDS")]
    now_ms: Option<i64>,

    #[arg(
        long = "suite",
        value_name = "SUITE",
        help = "Run test cases defined in a suite JSON file"
    )]
    suite: Option<PathBuf>,
}

#[derive(Debug)]
pub enum Cli {
    Check {
        function: PathBuf,
    },
    Run {
        function: PathBuf,
        event: PathBuf,
        kvs: Option<PathBuf>,
        now_ms: Option<i64>,
    },
    Test(TestInput),
}

#[derive(Debug)]
pub enum TestInput {
    Single {
        function: PathBuf,
        event: PathBuf,
        expected: PathBuf,
        kvs: Option<PathBuf>,
        now_ms: Option<i64>,
    },
    Suite {
        path: PathBuf,
    },
}

impl Cli {
    pub fn parse_args() -> AppResult<Self> {
        let raw = match RawCli::try_parse() {
            Ok(raw) => raw,
            Err(error) => match error.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => error.exit(),
                _ => return Err(AppError::Usage(error.to_string())),
            },
        };

        if let Some(path) = raw.suite {
            if raw.target != "test"
                || raw.function.is_some()
                || raw.event.is_some()
                || raw.expected.is_some()
                || raw.kvs.is_some()
                || raw.now_ms.is_some()
            {
                return Err(AppError::Usage(
                    "--suite is only allowed with test and cannot be combined with other test inputs"
                        .into(),
                ));
            }
            return Ok(Self::Test(TestInput::Suite { path }));
        }

        let target = raw.target;
        let function = match (target.as_str(), raw.function) {
            ("check" | "run" | "test", Some(function)) => function,
            ("check" | "run" | "test", None) => {
                return Err(AppError::Usage(format!(
                    "{} requires a function path",
                    target
                )));
            }
            (_, None) => PathBuf::from(&target),
            (_, Some(_)) => {
                return Err(AppError::Usage(
                    "a function path is only allowed after check, run, or test".into(),
                ));
            }
        };

        match target.as_str() {
            "check" => {
                if raw.event.is_some()
                    || raw.expected.is_some()
                    || raw.kvs.is_some()
                    || raw.now_ms.is_some()
                {
                    return Err(AppError::Usage("check accepts only a function path".into()));
                }
                Ok(Self::Check { function })
            }
            "run" => {
                let Some(event) = raw.event else {
                    return Err(AppError::Usage("run requires --event".into()));
                };
                if raw.expected.is_some() {
                    return Err(AppError::Usage("run does not accept --expected".into()));
                }
                Ok(Self::Run {
                    function,
                    event,
                    kvs: raw.kvs,
                    now_ms: raw.now_ms,
                })
            }
            "test" => {
                let Some(event) = raw.event else {
                    return Err(AppError::Usage("test requires --event".into()));
                };
                let Some(expected) = raw.expected else {
                    return Err(AppError::Usage("test requires --expected".into()));
                };
                Ok(Self::Test(TestInput::Single {
                    function,
                    event,
                    expected,
                    kvs: raw.kvs,
                    now_ms: raw.now_ms,
                }))
            }
            _ => Ok(Self::Test(TestInput::Single {
                function,
                event: raw
                    .event
                    .ok_or_else(|| AppError::Usage("test requires --event".into()))?,
                expected: raw
                    .expected
                    .ok_or_else(|| AppError::Usage("test requires --expected".into()))?,
                kvs: raw.kvs,
                now_ms: raw.now_ms,
            })),
        }
    }
}

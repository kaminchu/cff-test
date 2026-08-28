use std::path::PathBuf;

use clap::{Parser, error::ErrorKind};

use crate::error::{AppError, AppResult};

#[derive(Debug, Parser)]
#[command(
    name = "cff-test",
    version,
    about = "Test CloudFront Functions locally"
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    Check,
    Run,
    Test,
}

#[derive(Debug)]
pub struct Cli {
    pub command: Command,
    pub function: PathBuf,
    pub event: Option<PathBuf>,
    pub expected: Option<PathBuf>,
    pub kvs: Option<PathBuf>,
    pub now_ms: Option<i64>,
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
        let (command, function) = match (raw.target.as_str(), raw.function) {
            ("check", Some(function)) => (Command::Check, function),
            ("run", Some(function)) => (Command::Run, function),
            ("test", Some(function)) => (Command::Test, function),
            ("check" | "run" | "test", None) => {
                return Err(AppError::Usage(format!(
                    "{} requires a function path",
                    raw.target
                )));
            }
            (_, None) => (Command::Test, PathBuf::from(raw.target)),
            (_, Some(_)) => {
                return Err(AppError::Usage(
                    "a function path is only allowed after check, run, or test".into(),
                ));
            }
        };

        match command {
            Command::Check => {
                if raw.event.is_some()
                    || raw.expected.is_some()
                    || raw.kvs.is_some()
                    || raw.now_ms.is_some()
                {
                    return Err(AppError::Usage("check accepts only a function path".into()));
                }
            }
            Command::Run => {
                if raw.event.is_none() {
                    return Err(AppError::Usage("run requires --event".into()));
                }
                if raw.expected.is_some() {
                    return Err(AppError::Usage("run does not accept --expected".into()));
                }
            }
            Command::Test => {
                if raw.event.is_none() {
                    return Err(AppError::Usage("test requires --event".into()));
                }
                if raw.expected.is_none() {
                    return Err(AppError::Usage("test requires --expected".into()));
                }
            }
        }

        Ok(Self {
            command,
            function,
            event: raw.event,
            expected: raw.expected,
            kvs: raw.kvs,
            now_ms: raw.now_ms,
        })
    }
}

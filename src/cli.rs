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

    #[arg(short = 'i', long = "input", value_name = "EVENT")]
    input: Option<PathBuf>,

    #[arg(short = 'o', long = "output", value_name = "EXPECTED")]
    output: Option<PathBuf>,

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
    pub input: Option<PathBuf>,
    pub output: Option<PathBuf>,
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
                if raw.input.is_some()
                    || raw.output.is_some()
                    || raw.kvs.is_some()
                    || raw.now_ms.is_some()
                {
                    return Err(AppError::Usage("check accepts only a function path".into()));
                }
            }
            Command::Run => {
                if raw.input.is_none() {
                    return Err(AppError::Usage("run requires --input".into()));
                }
                if raw.output.is_some() {
                    return Err(AppError::Usage("run does not accept --output".into()));
                }
            }
            Command::Test => {
                if raw.input.is_none() {
                    return Err(AppError::Usage("test requires --input".into()));
                }
                if raw.output.is_none() {
                    return Err(AppError::Usage("test requires --output".into()));
                }
            }
        }

        Ok(Self {
            command,
            function,
            input: raw.input,
            output: raw.output,
            kvs: raw.kvs,
            now_ms: raw.now_ms,
        })
    }
}

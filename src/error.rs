use std::{io, path::PathBuf};

use thiserror::Error;

use crate::{checker::Diagnostic, event::ValidationError};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("usage error: {0}")]
    Usage(String),
    #[error("I/O error for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid JSON in {path} at line {line}, column {column}: {message}")]
    Json {
        path: PathBuf,
        line: usize,
        column: usize,
        message: String,
    },
    #[error("compatibility check failed:\n{}", format_diagnostics(.0))]
    Compatibility(Vec<Diagnostic>),
    #[error("event validation failed:\n{}", format_validation_errors(.0))]
    EventValidation(Vec<ValidationError>),
    #[error("runtime initialization failed: {0}")]
    RuntimeInit(String),
    #[error("JavaScript {phase} failed{suffix}: {message}{stack}", suffix = format_name(.name), stack = format_stack(.stack))]
    JavaScript {
        phase: String,
        name: Option<String>,
        message: String,
        stack: Option<String>,
    },
    #[error("local safety limit exceeded: {kind}")]
    LocalLimit { kind: String },
    #[error("return value validation failed:\n{}", format_validation_errors(.0))]
    ReturnValidation(Vec<ValidationError>),
    #[error("{0}")]
    Assertion(AssertionError),
}

#[derive(Debug, Error)]
#[error("JSON values differ ({count} differences)\n{details}")]
pub struct AssertionError {
    pub count: usize,
    pub details: String,
}

impl AppError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) | Self::Io { .. } | Self::Json { .. } => 2,
            _ => 1,
        }
    }
}

fn format_diagnostics(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_validation_errors(errors: &[ValidationError]) -> String {
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_name(name: &Option<String>) -> String {
    name.as_deref()
        .map_or(String::new(), |name| format!(" ({name})"))
}

fn format_stack(stack: &Option<String>) -> String {
    stack
        .as_deref()
        .map_or(String::new(), |stack| format!("\n{stack}"))
}

impl From<io::Error> for AppError {
    fn from(source: io::Error) -> Self {
        Self::Io {
            path: PathBuf::from("<unknown>"),
            source,
        }
    }
}

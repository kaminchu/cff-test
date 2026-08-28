mod diagnostic;
mod rules;
mod runtime20;

pub use diagnostic::Diagnostic;

use std::path::Path;

use crate::error::{AppError, AppResult};

pub struct CheckedSource {
    pub path: std::path::PathBuf,
    pub source: String,
}

pub fn check(path: &Path, source: &str) -> AppResult<CheckedSource> {
    let diagnostics = rules::check_source(path, source);
    if diagnostics.is_empty() {
        Ok(CheckedSource {
            path: path.to_path_buf(),
            source: source.into(),
        })
    } else {
        Err(AppError::Compatibility(diagnostics))
    }
}

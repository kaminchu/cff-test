use std::{fmt, path::PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
    pub rule: &'static str,
    pub message: String,
    pub help: String,
    pub byte_start: usize,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}:{}: error[{}]: {}\n  help: {}",
            self.path.display(),
            self.line,
            self.column,
            self.rule,
            self.message,
            self.help
        )
    }
}

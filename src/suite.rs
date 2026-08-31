use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use serde_json::Value;

use crate::{
    error::{AppError, AppResult},
    runtime::KvsFixture,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSuite {
    pub functions: Vec<RawSuiteFunction>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSuiteFunction {
    pub name: String,
    pub function: PathBuf,
    pub cases: Vec<RawSuiteCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawSuiteCase {
    pub name: String,
    pub event: JsonSource,
    pub expected: JsonSource,
    #[serde(default, deserialize_with = "deserialize_optional_json_source")]
    pub kvs: Option<JsonSource>,
    pub now_ms: Option<i64>,
    #[serde(default)]
    pub skip: bool,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum JsonSource {
    File(PathBuf),
    Inline(Value),
}

fn deserialize_optional_json_source<'de, D>(deserializer: D) -> Result<Option<JsonSource>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(JsonSource::deserialize(deserializer)?))
}

#[derive(Debug)]
pub struct Suite {
    pub functions: Vec<SuiteFunction>,
}

#[derive(Debug)]
pub struct SuiteFunction {
    pub name: String,
    pub function_path: PathBuf,
    pub source: String,
    pub cases: Vec<SuiteCase>,
}

#[derive(Debug)]
pub struct SuiteCase {
    pub name: String,
    pub event: Value,
    pub expected: Value,
    pub kvs: Option<KvsFixture>,
    pub now_ms: Option<i64>,
    pub skip: bool,
}

struct PendingSuiteFunction {
    name: String,
    function_path: PathBuf,
    source: String,
    cases: Vec<RawSuiteCase>,
}

impl Suite {
    pub fn from_path(path: &Path) -> AppResult<Self> {
        let suite_path = resolve_suite_path(path)?;
        let text = read_utf8(&suite_path)?;
        let raw: RawSuite = serde_json::from_str(&text).map_err(|error| AppError::Suite {
            path: suite_path.clone(),
            message: format!(
                "invalid JSON at line {}, column {}: {}",
                error.line(),
                error.column(),
                error
            ),
        })?;
        validate(&raw, &suite_path)?;

        let base = suite_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let mut pending = Vec::with_capacity(raw.functions.len());
        for function in raw.functions {
            let function_path = resolve_path(&base, &function.function);
            let source = read_utf8(&function_path)?;
            pending.push(PendingSuiteFunction {
                name: function.name,
                function_path,
                source,
                cases: function.cases,
            });
        }

        let mut functions = Vec::with_capacity(pending.len());
        for (function_index, function) in pending.into_iter().enumerate() {
            let mut cases = Vec::with_capacity(function.cases.len());
            for (case_index, case) in function.cases.into_iter().enumerate() {
                let event = resolve_json_source(case.event, &base)?;
                let expected = resolve_json_source(case.expected, &base)?;
                let kvs = case
                    .kvs
                    .map(|source| {
                        resolve_kvs_source(source, &base, &suite_path, function_index, case_index)
                    })
                    .transpose()?;
                cases.push(SuiteCase {
                    name: case.name,
                    event,
                    expected,
                    kvs,
                    now_ms: case.now_ms,
                    skip: case.skip,
                });
            }
            functions.push(SuiteFunction {
                name: function.name,
                function_path: function.function_path,
                source: function.source,
                cases,
            });
        }

        Ok(Self { functions })
    }
}

fn validate(raw: &RawSuite, path: &Path) -> AppResult<()> {
    if raw.functions.is_empty() {
        return suite_error(path, "functions must not be empty");
    }
    let mut function_names = HashSet::new();
    for (function_index, function) in raw.functions.iter().enumerate() {
        validate_name(
            &function.name,
            &format!("functions[{function_index}].name"),
            path,
        )?;
        if !function_names.insert(&function.name) {
            return suite_error(path, &format!("duplicate function name: {}", function.name));
        }
        if function.cases.is_empty() {
            return suite_error(
                path,
                &format!("functions[{function_index}].cases must not be empty"),
            );
        }
        let mut case_names = HashSet::new();
        for (case_index, case) in function.cases.iter().enumerate() {
            validate_name(
                &case.name,
                &format!("functions[{function_index}].cases[{case_index}].name"),
                path,
            )?;
            if !case_names.insert(&case.name) {
                return suite_error(
                    path,
                    &format!(
                        "duplicate case name in function {}: {}",
                        function.name, case.name
                    ),
                );
            }
        }
    }
    Ok(())
}

fn validate_name(name: &str, field: &str, path: &Path) -> AppResult<()> {
    if name.trim().is_empty() {
        return suite_error(path, &format!("{field} must not be empty or whitespace"));
    }
    if name.contains('\n') || name.contains('\r') {
        return suite_error(path, &format!("{field} must not contain a newline"));
    }
    Ok(())
}

fn resolve_json_source(source: JsonSource, base: &Path) -> AppResult<Value> {
    match source {
        JsonSource::File(path) => read_json(&resolve_path(base, &path)),
        JsonSource::Inline(value) => Ok(value),
    }
}

fn resolve_kvs_source(
    source: JsonSource,
    base: &Path,
    suite_path: &Path,
    function_index: usize,
    case_index: usize,
) -> AppResult<KvsFixture> {
    match source {
        JsonSource::File(path) => KvsFixture::from_path(&resolve_path(base, &path)),
        JsonSource::Inline(value) => KvsFixture::from_value(
            value,
            PathBuf::from(format!(
                "{}#functions[{function_index}].cases[{case_index}].kvs",
                suite_path.display()
            )),
        ),
    }
}

fn resolve_suite_path(path: &Path) -> AppResult<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let current_dir = std::env::current_dir().map_err(|source| AppError::Io {
        path: PathBuf::from("."),
        source,
    })?;
    Ok(current_dir.join(path))
}

fn resolve_path(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
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
        message: "file is not valid UTF-8".into(),
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

fn suite_error<T>(path: &Path, message: &str) -> AppResult<T> {
    Err(AppError::Suite {
        path: path.to_path_buf(),
        message: message.into(),
    })
}

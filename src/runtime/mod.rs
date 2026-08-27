mod buffer;
mod cloudfront;
mod crypto;
mod host;
mod module_loader;

use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use rquickjs::prelude::Func;
use rquickjs::{CaughtError, Context, Ctx, Function, Module, Runtime, Value};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};

use crate::{
    checker::CheckedSource,
    error::{AppError, AppResult},
};

const MEMORY_LIMIT: usize = 64 * 1024 * 1024;
const STACK_LIMIT: usize = 512 * 1024;
const TIME_LIMIT: Duration = Duration::from_secs(1);

pub struct RuntimeOptions {
    pub now_ms: Option<i64>,
    pub kvs: Option<KvsFixture>,
}

#[derive(Clone, Debug)]
pub struct KvsFixture {
    values: BTreeMap<String, KvsValue>,
    meta: KvsMeta,
}
#[derive(Clone, Debug)]
struct KvsValue {
    format: String,
    value: JsonValue,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct KvsMeta {
    creation_date_time: String,
    last_updated_date_time: String,
    key_count: usize,
}
#[derive(Debug, Deserialize)]
struct RawKvs {
    values: BTreeMap<String, RawKvsValue>,
    meta: KvsMeta,
}
#[derive(Debug, Deserialize)]
struct RawKvsValue {
    format: String,
    value: JsonValue,
}

impl KvsFixture {
    pub fn from_path(path: &Path) -> AppResult<Self> {
        let text = fs::read_to_string(path).map_err(|source| AppError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let raw: RawKvs = serde_json::from_str(&text).map_err(|error| AppError::Json {
            path: path.to_path_buf(),
            line: error.line(),
            column: error.column(),
            message: error.to_string(),
        })?;
        if raw.meta.key_count != raw.values.len() {
            return Err(AppError::Json {
                path: path.to_path_buf(),
                line: 1,
                column: 1,
                message: "meta.keyCount must equal the number of values".into(),
            });
        }
        let mut values = BTreeMap::new();
        for (key, value) in raw.values {
            if !matches!(value.format.as_str(), "string" | "json" | "bytes") {
                return Err(AppError::Json {
                    path: path.to_path_buf(),
                    line: 1,
                    column: 1,
                    message: format!("values.{key}.format must be string, json, or bytes"),
                });
            }
            if matches!(value.format.as_str(), "string" | "bytes") && !value.value.is_string() {
                return Err(AppError::Json {
                    path: path.to_path_buf(),
                    line: 1,
                    column: 1,
                    message: format!(
                        "values.{key}.value must be a string for {} format",
                        value.format
                    ),
                });
            }
            if value.format == "bytes" && STANDARD.decode(value.value.as_str().unwrap()).is_err() {
                return Err(AppError::Json {
                    path: path.to_path_buf(),
                    line: 1,
                    column: 1,
                    message: format!("values.{key}.value must be valid standard base64"),
                });
            }
            values.insert(
                key,
                KvsValue {
                    format: value.format,
                    value: value.value,
                },
            );
        }
        Ok(Self {
            values,
            meta: raw.meta,
        })
    }
    fn as_json(&self) -> JsonValue {
        let values = self
            .values
            .iter()
            .map(|(key, value)| {
                (
                    key.clone(),
                    serde_json::json!({"format": value.format, "value": value.value}),
                )
            })
            .collect::<Map<_, _>>();
        serde_json::json!({"values": values, "meta": self.meta})
    }
}

pub struct RuntimeRunner;
impl RuntimeRunner {
    pub fn execute(
        source: &CheckedSource,
        event: &JsonValue,
        options: RuntimeOptions,
    ) -> AppResult<JsonValue> {
        let now_ms = options.now_ms.unwrap_or_else(current_epoch_ms);
        let deadline = std::time::Instant::now() + TIME_LIMIT;
        let runtime = Runtime::new().map_err(|error| AppError::RuntimeInit(error.to_string()))?;
        runtime.set_memory_limit(MEMORY_LIMIT);
        runtime.set_max_stack_size(STACK_LIMIT);
        runtime.set_interrupt_handler(Some(Box::new(move || {
            std::time::Instant::now() >= deadline
        })));
        let loader = module_loader::loader();
        let resolver = module_loader::resolver();
        runtime.set_loader(resolver, loader);
        let context =
            Context::full(&runtime).map_err(|error| AppError::RuntimeInit(error.to_string()))?;
        let result =
            context.with(|ctx| Self::execute_in_context(ctx, source, event, options, now_ms));
        match result {
            Err(_) if std::time::Instant::now() >= deadline => Err(AppError::LocalLimit {
                kind: "JavaScript execution exceeded the 1 second local safety limit".into(),
            }),
            other => other,
        }
    }
    fn execute_in_context<'js>(
        ctx: Ctx<'js>,
        source: &CheckedSource,
        event: &JsonValue,
        options: RuntimeOptions,
        now_ms: i64,
    ) -> AppResult<JsonValue> {
        buffer::init(&ctx).map_err(|error| AppError::RuntimeInit(error.to_string()))?;
        crypto::init(&ctx).map_err(|error| AppError::RuntimeInit(error.to_string()))?;
        ctx.globals()
            .set("__cff_now_ms", now_ms)
            .map_err(|error| AppError::RuntimeInit(error.to_string()))?;
        let fixture = options
            .kvs
            .as_ref()
            .map(KvsFixture::as_json)
            .unwrap_or(JsonValue::Null);
        ctx.globals()
            .set("__cff_kvs_json", serde_json::to_string(&fixture).unwrap())
            .map_err(|error| AppError::RuntimeInit(error.to_string()))?;
        ctx.globals()
            .set("__cff_kvs_associated", options.kvs.is_some())
            .map_err(|error| AppError::RuntimeInit(error.to_string()))?;
        ctx.globals()
            .set("__cff_console_log", Func::from(host_console_log))
            .map_err(|error| AppError::RuntimeInit(error.to_string()))?;
        ctx.eval::<(), _>(include_str!("bootstrap.js"))
            .map_err(|error| js_error(&ctx, "bootstrap", error))?;
        for (name, module_source) in [
            ("crypto", include_str!("crypto.js")),
            ("querystring", include_str!("querystring.js")),
            ("cloudfront", include_str!("cloudfront.js")),
        ] {
            let promise = Module::evaluate(ctx.clone(), name, module_source)
                .map_err(|error| js_error(&ctx, "module evaluation", error))?;
            promise
                .finish::<Value>()
                .map_err(|error| js_error(&ctx, "module evaluation", error))?;
        }
        ctx.eval::<(), _>(
            "globalThis.require = ((crypto, querystring, cloudfront) => (name) => { if (name === \"crypto\") return crypto; if (name === \"querystring\") return querystring; if (name === \"cloudfront\") return cloudfront; throw new Error(\"module is not available: \" + name); })(globalThis.__cff_crypto, globalThis.__cff_querystring, globalThis.__cff_cloudfront); delete globalThis.__cff_now_ms; delete globalThis.__cff_kvs_json; delete globalThis.__cff_kvs_associated; delete globalThis.__cff_console_log; delete globalThis.__cff_digest; delete globalThis.__cff_hmac_digest; delete globalThis.__cff_crypto; delete globalThis.__cff_querystring; delete globalThis.__cff_cloudfront;",
        )
        .map_err(|error| js_error(&ctx, "bootstrap cleanup", error))?;
        let source_text = format!("{}\nexport {{ handler as __cff_handler }};", source.source);
        let declared = Module::declare(ctx.clone(), source.path.display().to_string(), source_text)
            .map_err(|error| js_error(&ctx, "module evaluation", error))?;
        let (evaluated, module_promise) = declared
            .eval()
            .map_err(|error| js_error(&ctx, "module evaluation", error))?;
        module_promise
            .finish::<Value>()
            .map_err(|error| js_error(&ctx, "module evaluation", error))?;
        let handler: Function = evaluated
            .get("__cff_handler")
            .map_err(|error| js_error(&ctx, "module evaluation", error))?;
        ctx.globals()
            .set("__cff_event_json", serde_json::to_string(event).unwrap())
            .map_err(|error| AppError::RuntimeInit(error.to_string()))?;
        ctx.eval::<(), _>("globalThis.__cff_event = JSON.parse(globalThis.__cff_event_json);")
            .map_err(|error| js_error(&ctx, "event injection", error))?;
        let event_value: Value = ctx
            .globals()
            .get("__cff_event")
            .map_err(|error| js_error(&ctx, "event injection", error))?;
        ctx.eval::<(), _>("delete globalThis.__cff_event_json; delete globalThis.__cff_event;")
            .map_err(|error| js_error(&ctx, "event injection", error))?;
        let returned: Value = handler
            .call((event_value,))
            .map_err(|error| js_error(&ctx, "handler invocation", error))?;
        let returned = if returned.is_promise() {
            returned
                .into_promise()
                .expect("promise value")
                .finish::<Value>()
                .map_err(|error| js_error(&ctx, "promise settlement", error))?
        } else {
            returned
        };
        ctx.globals()
            .set("__cff_return", returned)
            .map_err(|error| js_error(&ctx, "serialization", error))?;
        let serialized: Option<String> = ctx
            .eval("JSON.stringify(globalThis.__cff_return)")
            .map_err(|error| js_error(&ctx, "serialization", error))?;
        let serialized = serialized.ok_or_else(|| AppError::JavaScript {
            phase: "serialization".into(),
            name: None,
            message: "handler returned a value that cannot be represented as JSON".into(),
            stack: None,
        })?;
        serde_json::from_str(&serialized).map_err(|error| AppError::JavaScript {
            phase: "serialization".into(),
            name: None,
            message: error.to_string(),
            stack: None,
        })
    }
}
fn current_epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
fn js_error<'js>(ctx: &Ctx<'js>, phase: &str, error: rquickjs::Error) -> AppError {
    match CaughtError::from_error(ctx, error) {
        CaughtError::Exception(exception) => AppError::JavaScript {
            phase: phase.into(),
            name: None,
            message: exception
                .message()
                .unwrap_or_else(|| "JavaScript exception".into()),
            stack: exception.stack(),
        },
        CaughtError::Value(value) => AppError::JavaScript {
            phase: phase.into(),
            name: None,
            message: format!("handler threw a non-Error value: {value:?}"),
            stack: None,
        },
        CaughtError::Error(error) => AppError::JavaScript {
            phase: phase.into(),
            name: None,
            message: error.to_string(),
            stack: None,
        },
    }
}
pub(crate) fn host_console_log(value: rquickjs::Coerced<String>) -> rquickjs::Result<()> {
    eprintln!("{}", value.0);
    Ok(())
}

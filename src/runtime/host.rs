use llrt_buffer::ArrayBufferView;
use rquickjs::function::Opt;
use rquickjs::{Ctx, Result, Value};

use super::crypto;

pub(crate) fn host_digest<'js>(
    ctx: Ctx<'js>,
    algorithm: String,
    data: ArrayBufferView<'js>,
    encoding: Opt<String>,
) -> Result<Value<'js>> {
    crypto::digest_value(
        &ctx,
        &algorithm,
        &[],
        data.as_bytes()
            .ok_or_else(|| rquickjs::Error::new_from_js("value", "ArrayBufferView"))?,
        encoding.0.as_deref(),
    )
}

pub(crate) fn host_hmac_digest<'js>(
    ctx: Ctx<'js>,
    algorithm: String,
    key: ArrayBufferView<'js>,
    data: ArrayBufferView<'js>,
    encoding: Opt<String>,
) -> Result<Value<'js>> {
    crypto::digest_value(
        &ctx,
        &algorithm,
        key.as_bytes()
            .ok_or_else(|| rquickjs::Error::new_from_js("value", "ArrayBufferView"))?,
        data.as_bytes()
            .ok_or_else(|| rquickjs::Error::new_from_js("value", "ArrayBufferView"))?,
        encoding.0.as_deref(),
    )
}

use super::host::{host_digest, host_hmac_digest};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use llrt_buffer::Buffer;
use md5::Digest;
use md5::Md5;
use rquickjs::IntoJs;
use rquickjs::prelude::Func;
use rquickjs::{Ctx, Result, Value};
use sha1::Sha1;
use sha2::Sha256;
pub fn init<'js>(ctx: &Ctx<'js>) -> Result<()> {
    ctx.globals().set("__cff_digest", Func::from(host_digest))?;
    ctx.globals()
        .set("__cff_hmac_digest", Func::from(host_hmac_digest))?;
    Ok(())
}
pub fn digest_value<'js>(
    ctx: &Ctx<'js>,
    algorithm: &str,
    key: &[u8],
    data: &[u8],
    encoding: Option<&str>,
) -> Result<Value<'js>> {
    let digest = if key.is_empty() {
        hash(algorithm, data)?
    } else {
        hmac(algorithm, key, data)?
    };
    match encoding {
        None => Buffer(digest).into_js(ctx),
        Some("hex") => Ok(hex(&digest).into_js(ctx)?),
        Some("base64") => Ok(STANDARD.encode(digest).into_js(ctx)?),
        Some("base64url") => Ok(URL_SAFE_NO_PAD.encode(digest).into_js(ctx)?),
        Some(_) => Err(rquickjs::Error::new_from_js(
            "encoding",
            "supported encoding",
        )),
    }
}
fn hash(algorithm: &str, data: &[u8]) -> Result<Vec<u8>> {
    match algorithm {
        "md5" => Ok(Md5::digest(data).to_vec()),
        "sha1" => Ok(Sha1::digest(data).to_vec()),
        "sha256" => Ok(Sha256::digest(data).to_vec()),
        _ => Err(rquickjs::Error::new_from_js(
            "algorithm",
            "md5, sha1, or sha256",
        )),
    }
}
fn hmac(algorithm: &str, key: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    const BLOCK: usize = 64;
    let mut normalized = if key.len() > BLOCK {
        hash(algorithm, key)?
    } else {
        key.to_vec()
    };
    normalized.resize(BLOCK, 0);
    let mut inner = vec![0x36; BLOCK];
    let mut outer = vec![0x5c; BLOCK];
    for index in 0..BLOCK {
        inner[index] ^= normalized[index];
        outer[index] ^= normalized[index];
    }
    inner.extend_from_slice(data);
    outer.extend_from_slice(&hash(algorithm, &inner)?);
    hash(algorithm, &outer)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

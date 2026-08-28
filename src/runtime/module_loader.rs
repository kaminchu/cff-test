use rquickjs::loader::{BuiltinLoader, BuiltinResolver};
pub fn resolver() -> BuiltinResolver {
    BuiltinResolver::default()
        .with_module("crypto")
        .with_module("querystring")
        .with_module("cloudfront")
}
pub fn loader() -> BuiltinLoader {
    BuiltinLoader::default()
        .with_module("crypto", include_str!("crypto.js"))
        .with_module("querystring", include_str!("querystring.js"))
        .with_module("cloudfront", include_str!("cloudfront.js"))
}

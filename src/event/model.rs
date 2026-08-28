use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub version: String,
    pub context: Context,
    pub viewer: Viewer,
    pub request: Request,
    pub response: Option<Response>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub event_type: String,
    pub distribution_domain_name: Option<String>,
    pub distribution_id: Option<String>,
    pub request_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Viewer {
    pub ip: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Request {
    pub method: String,
    pub uri: String,
    pub querystring: serde_json::Value,
    pub headers: serde_json::Value,
    pub cookies: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub status_code: i64,
    pub status_description: Option<String>,
    pub headers: serde_json::Value,
    pub cookies: serde_json::Value,
    pub body: Option<serde_json::Value>,
}

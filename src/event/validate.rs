use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Map, Value};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

pub fn validate_event(event: &Value) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    let Some(root) = event.as_object() else {
        return Err(vec![error("/", "event must be an object")]);
    };
    require_string(root, "version", "/version", &mut errors);
    if root.get("version").and_then(Value::as_str) != Some("1.0") {
        errors.push(error("/version", "must be \"1.0\""));
    }
    let event_type = root
        .get("context")
        .and_then(Value::as_object)
        .and_then(|context| context.get("eventType"))
        .and_then(Value::as_str);
    let Some(context) = root.get("context").and_then(Value::as_object) else {
        errors.push(error("/context", "is required and must be an object"));
        return Err(errors);
    };
    require_string(context, "eventType", "/context/eventType", &mut errors);
    if !matches!(event_type, Some("viewer-request" | "viewer-response")) {
        errors.push(error(
            "/context/eventType",
            "must be viewer-request or viewer-response",
        ));
    }
    validate_viewer(root.get("viewer"), &mut errors);
    validate_request(root.get("request"), "/request", &mut errors);

    match event_type {
        Some("viewer-request") => {
            if root.get("response").is_some() {
                errors.push(error("/response", "must be omitted for viewer-request"));
            }
        }
        Some("viewer-response") => {
            validate_response(root.get("response"), "/response", &mut errors)
        }
        _ => {}
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn validate_return(
    value: &Value,
    event_type: Option<&Value>,
    input_method: Option<&str>,
) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    let event_type = event_type.and_then(Value::as_str);
    let Some(object) = value.as_object() else {
        return Err(vec![error("/", "handler return value must be an object")]);
    };

    let is_response = object.contains_key("statusCode");
    if event_type == Some("viewer-response") && !is_response {
        errors.push(error(
            "/",
            "viewer-response handler must return a response object",
        ));
    }
    if event_type == Some("viewer-request") && !is_response {
        validate_request(Some(value), "/", &mut errors);
        if let Some(method) = object.get("method").and_then(Value::as_str)
            && input_method != Some(method)
        {
            errors.push(error(
                "/method",
                "request method is read-only and must not be changed",
            ));
        }
    }
    if is_response {
        validate_response(Some(value), "/", &mut errors);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_viewer(value: Option<&Value>, errors: &mut Vec<ValidationError>) {
    let Some(object) = required_object(value, "/viewer", errors) else {
        return;
    };
    require_string(object, "ip", "/viewer/ip", errors);
}

fn validate_request(value: Option<&Value>, path: &str, errors: &mut Vec<ValidationError>) {
    let Some(object) = required_object(value, path, errors) else {
        return;
    };
    require_string(object, "method", &format!("{path}/method"), errors);
    if let Some(uri) = object.get("uri") {
        if uri.as_str().is_none() || !uri.as_str().is_some_and(|uri| uri.starts_with('/')) {
            errors.push(error(
                &format!("{path}/uri"),
                "must be a string beginning with /",
            ));
        }
    } else {
        errors.push(error(&format!("{path}/uri"), "is required"));
    }
    for name in ["querystring", "headers", "cookies"] {
        let item_path = format!("{path}/{name}");
        match object.get(name) {
            Some(value) if value.is_object() => validate_entries(
                value.as_object().unwrap(),
                &item_path,
                name == "headers",
                errors,
            ),
            Some(_) => errors.push(error(&item_path, "must be an object")),
            None => errors.push(error(&item_path, "is required")),
        }
    }
    if object.contains_key("body") {
        errors.push(error(
            &format!("{path}/body"),
            "request body is not available in CloudFront Functions",
        ));
    }
}

fn validate_response(value: Option<&Value>, path: &str, errors: &mut Vec<ValidationError>) {
    let Some(object) = required_object(value, path, errors) else {
        return;
    };
    match object.get("statusCode").and_then(Value::as_i64) {
        Some(code) if (100..=599).contains(&code) => {}
        Some(_) => errors.push(error(
            &format!("{path}/statusCode"),
            "must be an integer from 100 through 599",
        )),
        None => errors.push(error(
            &format!("{path}/statusCode"),
            "is required and must be an integer",
        )),
    }
    if let Some(description) = object.get("statusDescription")
        && !description.is_string()
    {
        errors.push(error(
            &format!("{path}/statusDescription"),
            "must be a string",
        ));
    }
    for name in ["headers", "cookies"] {
        let item_path = format!("{path}/{name}");
        match object.get(name) {
            Some(value) if value.is_object() => validate_entries(
                value.as_object().unwrap(),
                &item_path,
                name == "headers",
                errors,
            ),
            Some(_) => errors.push(error(&item_path, "must be an object")),
            None => errors.push(error(&item_path, "is required")),
        }
    }
    if let Some(body) = object.get("body") {
        match body {
            Value::String(_) => {}
            Value::Object(body) => {
                let encoding = body.get("encoding").and_then(Value::as_str);
                let data = body.get("data").and_then(Value::as_str);
                if !matches!(encoding, Some("text" | "base64")) {
                    errors.push(error(
                        &format!("{path}/body/encoding"),
                        "must be text or base64",
                    ));
                }
                if let Some(data) = data {
                    if encoding == Some("base64") && STANDARD.decode(data).is_err() {
                        errors.push(error(
                            &format!("{path}/body/data"),
                            "must be valid standard base64",
                        ));
                    }
                } else {
                    errors.push(error(
                        &format!("{path}/body/data"),
                        "is required and must be a string",
                    ));
                }
            }
            _ => errors.push(error(
                &format!("{path}/body"),
                "must be a string or an encoding/data object",
            )),
        }
    }
}

fn validate_entries(
    object: &Map<String, Value>,
    path: &str,
    headers: bool,
    errors: &mut Vec<ValidationError>,
) {
    for (name, entry) in object {
        if headers && (!name.is_ascii() || name.bytes().any(|byte| byte.is_ascii_uppercase())) {
            errors.push(error(
                &format!("{path}/{}", pointer_escape(name)),
                "header names must be ASCII lowercase",
            ));
        }
        let Some(entry) = entry.as_object() else {
            errors.push(error(
                &format!("{path}/{}", pointer_escape(name)),
                "must be an object",
            ));
            continue;
        };
        if entry.get("value").and_then(Value::as_str).is_none() {
            errors.push(error(
                &format!("{path}/{}/value", pointer_escape(name)),
                "is required and must be a string",
            ));
        }
        if let Some(multi) = entry.get("multiValue") {
            let Some(values) = multi.as_array() else {
                errors.push(error(
                    &format!("{path}/{}/multiValue", pointer_escape(name)),
                    "must be an array",
                ));
                continue;
            };
            if values.is_empty() {
                errors.push(error(
                    &format!("{path}/{}/multiValue", pointer_escape(name)),
                    "must not be empty",
                ));
            }
            for (index, value) in values.iter().enumerate() {
                if value.get("value").and_then(Value::as_str).is_none() {
                    errors.push(error(
                        &format!("{path}/{}/{index}/value", pointer_escape(name)),
                        "is required and must be a string",
                    ));
                }
            }
        }
    }
}

fn required_object<'a>(
    value: Option<&'a Value>,
    path: &str,
    errors: &mut Vec<ValidationError>,
) -> Option<&'a Map<String, Value>> {
    match value.and_then(Value::as_object) {
        Some(object) => Some(object),
        None => {
            errors.push(error(path, "is required and must be an object"));
            None
        }
    }
}

fn require_string(
    object: &Map<String, Value>,
    name: &str,
    path: &str,
    errors: &mut Vec<ValidationError>,
) {
    if object.get(name).and_then(Value::as_str).is_none() {
        errors.push(error(path, "is required and must be a string"));
    }
}

fn error(path: &str, message: &str) -> ValidationError {
    ValidationError {
        path: path.into(),
        message: message.into(),
    }
}

fn pointer_escape(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

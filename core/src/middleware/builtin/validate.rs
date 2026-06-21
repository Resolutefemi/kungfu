//! JSON Schema request validation middleware.
//!
//! Validates the request body against a JSON Schema before the handler runs.
//! If validation fails, the framework returns 422 Unprocessable Entity with
//! a structured error message.
//!
//! ## Example
//!
//! ```ignore
//! use kungfu::middleware_builtin::validate_json;
//! use serde_json::json;
//!
//! let user_schema = json!({
//!     "type": "object",
//!     "properties": {
//!         "email": {"type": "string", "format": "email"},
//!         "password": {"type": "string", "minLength": 8}
//!     },
//!     "required": ["email", "password"]
//! });
//!
//! Kungfu::new()
//!     .use_middleware(validate_json("/users", kungfu::Method::Post, user_schema))
//!     .handle_post("/users", |_req, res| res.text("created"))
//! ```

use std::sync::Arc;

use crate::middleware::{Middleware, Next};
use crate::request::Request;
use crate::response::Response;

/// Create a JSON Schema validation middleware for a specific route.
///
/// The middleware only kicks in for the given (path, method) combination.
/// Other routes pass through untouched.
pub fn validate_json(path: &str, method: crate::Method, schema: serde_json::Value) -> Middleware {
    let path = path.to_string();
    let schema = Arc::new(schema);
    Arc::new(move |req: Request, next: Next| {
        let path = path.clone();
        let schema = schema.clone();
        let target_method = method;
        Box::pin(async move {
            // Only validate the specified route.
            if req.path != path || req.method != target_method {
                return next(req).await;
            }
            // Validate the body.
            if let Err(errors) = validate_against_schema(&req.body, &schema) {
                return Response::new()
                    .status(crate::StatusCode::UnprocessableEntity)
                    .json(&serde_json::json!({
                        "error": {
                            "code": 422,
                            "message": "Validation failed",
                            "detail": errors,
                            "suggestion": "Fix the listed fields and retry.",
                        }
                    }));
            }
            next(req).await
        })
    })
}

/// Validate a JSON body against a JSON Schema. Returns `Ok(())` if valid,
/// `Err(Vec<String>)` with a list of error messages otherwise.
///
/// V1 supports a subset of JSON Schema: `type`, `properties`, `required`,
/// `items`, `minLength`, `maxLength`, `minimum`, `maximum`, `enum`. Full
/// JSON Schema 2020-12 support is planned for V1.1.
pub fn validate_against_schema(body: &[u8], schema: &serde_json::Value) -> Result<(), Vec<String>> {
    let value: serde_json::Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(e) => return Err(vec![format!("Invalid JSON: {e}")]),
    };
    let mut errors = Vec::new();
    validate_value(&value, schema, "$", &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_value(value: &serde_json::Value, schema: &serde_json::Value, path: &str, errors: &mut Vec<String>) {
    // type
    if let Some(expected_type) = schema.get("type").and_then(|v| v.as_str()) {
        let actual_type = json_type(value);
        if !type_matches(actual_type, expected_type) {
            errors.push(format!("{path}: expected type {expected_type}, got {actual_type}"));
            return;
        }
    }

    // enum
    if let Some(serde_json::Value::Array(allowed)) = schema.get("enum") {
        if !allowed.contains(value) {
            errors.push(format!("{path}: value not in allowed enum"));
        }
    }

    // Object validation.
    if let (Some(obj), Some(props)) = (value.as_object(), schema.get("properties")) {
        // required
        if let Some(required) = schema.get("required").and_then(|v| v.as_array()) {
            for req in required {
                if let Some(name) = req.as_str() {
                    if !obj.contains_key(name) {
                        errors.push(format!("{path}: missing required field '{name}'"));
                    }
                }
            }
        }
        // Validate each property.
        if let Some(props_obj) = props.as_object() {
            for (key, sub_schema) in props_obj {
                if let Some(sub_value) = obj.get(key) {
                    validate_value(sub_value, sub_schema, &format!("{path}.{key}"), errors);
                }
            }
        }
    }

    // Array validation.
    if let (Some(arr), Some(items_schema)) = (value.as_array(), schema.get("items")) {
        for (i, item) in arr.iter().enumerate() {
            validate_value(item, items_schema, &format!("{path}[{i}]"), errors);
        }
    }

    // String constraints.
    if let Some(s) = value.as_str() {
        if let Some(min) = schema.get("minLength").and_then(|v| v.as_u64()) {
            if (s.len() as u64) < min {
                errors.push(format!("{path}: string too short (min {min} chars)"));
            }
        }
        if let Some(max) = schema.get("maxLength").and_then(|v| v.as_u64()) {
            if (s.len() as u64) > max {
                errors.push(format!("{path}: string too long (max {max} chars)"));
            }
        }
    }

    // Number constraints.
    if let Some(n) = value.as_f64() {
        if let Some(min) = schema.get("minimum").and_then(|v| v.as_f64()) {
            if n < min {
                errors.push(format!("{path}: value {n} below minimum {min}"));
            }
        }
        if let Some(max) = schema.get("maximum").and_then(|v| v.as_f64()) {
            if n > max {
                errors.push(format!("{path}: value {n} above maximum {max}"));
            }
        }
    }
}

fn json_type(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn type_matches(actual: &str, expected: &str) -> bool {
    if actual == expected {
        return true;
    }
    // JSON Schema allows "integer" to be a subtype of "number".
    if expected == "number" && actual == "integer" {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validates_required_fields() {
        let schema = json!({
            "type": "object",
            "properties": {
                "email": {"type": "string"},
                "password": {"type": "string", "minLength": 8}
            },
            "required": ["email", "password"]
        });
        let body = br#"{"email":"a@b.com"}"#;
        let result = validate_against_schema(body, &schema);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("missing required field 'password'")));
    }

    #[test]
    fn validates_min_length() {
        let schema = json!({
            "type": "object",
            "properties": {
                "password": {"type": "string", "minLength": 8}
            },
            "required": ["password"]
        });
        let body = br#"{"password":"short"}"#; // 5 chars
        let result = validate_against_schema(body, &schema);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("string too short")));
    }

    #[test]
    fn passes_valid_body() {
        let schema = json!({
            "type": "object",
            "properties": {
                "email": {"type": "string"},
                "password": {"type": "string", "minLength": 8}
            },
            "required": ["email", "password"]
        });
        let body = br#"{"email":"a@b.com","password":"longenough"}"#;
        let result = validate_against_schema(body, &schema);
        assert!(result.is_ok());
    }

    #[test]
    fn rejects_invalid_json() {
        let schema = json!({"type": "object"});
        let body = b"not valid json";
        let result = validate_against_schema(body, &schema);
        assert!(result.is_err());
    }

    #[test]
    fn validates_number_range() {
        let schema = json!({
            "type": "object",
            "properties": {
                "age": {"type": "number", "minimum": 0, "maximum": 150}
            }
        });
        let body = br#"{"age": 200}"#;
        let result = validate_against_schema(body, &schema);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("above maximum")));
    }
}

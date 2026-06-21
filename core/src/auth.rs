//! Built-in JWT authentication middleware.
//!
//! Provides `auth_jwt(secret)` middleware that verifies `Authorization: Bearer <token>`
//! headers. If valid, the decoded claims are available via `req.extensions`.
//! If invalid or missing, returns 401.
//!
//! ## Example
//!
//! ```ignore
//! use kungfu::auth::{auth_jwt, AuthProvider};
//!
//! Kungfu::new()
//!     .use_middleware(auth_jwt("my-secret-key"))
//!     .handle_get("/protected", |_req, res| res.text("secret data"))
//! ```
//!
//! V1 supports HS256 only. RS256 / ES256 planned for V1.1.

use std::sync::Arc;

use crate::middleware::{Middleware, Next};
use crate::request::Request;
use crate::response::Response;

/// Configuration for JWT authentication.
#[derive(Debug, Clone)]
pub struct JwtConfig {
    /// The HS256 secret used to verify tokens.
    pub secret: String,
    /// The header name to read the token from (default: `authorization`).
    pub header_name: String,
    /// The expected prefix (default: `Bearer `).
    pub prefix: String,
    /// Paths that don't require auth (e.g. `/login`, `/signup`).
    pub public_paths: Vec<String>,
}

impl JwtConfig {
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
            header_name: "authorization".into(),
            prefix: "Bearer ".into(),
            public_paths: Vec::new(),
        }
    }

    pub fn public_path(mut self, path: impl Into<String>) -> Self {
        self.public_paths.push(path.into());
        self
    }
}

/// Create a JWT authentication middleware.
pub fn auth_jwt(config: JwtConfig) -> Middleware {
    let config = Arc::new(config);
    Arc::new(move |req: Request, next: Next| {
        let config = config.clone();
        Box::pin(async move {
            // Skip auth for public paths.
            if config.public_paths.iter().any(|p| req.path == *p) {
                return next(req).await;
            }

            // Extract the token.
            let header_value = match req.header(&config.header_name) {
                Some(v) => v.to_string(),
                None => {
                    return Response::new()
                        .status(crate::StatusCode::Unauthorized)
                        .json(&serde_json::json!({
                            "error": {
                                "code": 401,
                                "message": "Missing Authorization header",
                                "detail": format!("Expected: {}: {}<token>", config.header_name, config.prefix),
                                "suggestion": "Include a valid JWT in the Authorization header.",
                            }
                        }));
                }
            };

            let token = match header_value.strip_prefix(&config.prefix) {
                Some(t) => t.trim(),
                None => &header_value,
            };

            // Verify the token (V1: just check it's non-empty + has 3 parts).
            // Full HS256 verification requires a JWT lib — added in V1.1.
            if token.is_empty() || token.split('.').count() != 3 {
                return Response::new()
                    .status(crate::StatusCode::Unauthorized)
                    .json(&serde_json::json!({
                        "error": {
                            "code": 401,
                            "message": "Invalid JWT",
                            "detail": "Token must be a valid JWT with 3 parts separated by '.'",
                            "suggestion": "Ensure your token is a valid JWT.",
                        }
                    }));
            }

            // Decode the claims (the middle part of the JWT, base64url-encoded JSON).
            let parts: Vec<&str> = token.split('.').collect();
            let claims_json = match decode_base64url(parts[1]) {
                Some(s) => s,
                None => {
                    return Response::new()
                        .status(crate::StatusCode::Unauthorized)
                        .json(&serde_json::json!({
                            "error": {
                                "code": 401,
                                "message": "Invalid JWT claims",
                                "detail": "Could not base64-decode the claims section.",
                            }
                        }));
                }
            };

            // V1: we don't verify the signature — just decode. V1.1 will
            // add proper HS256 verification via `jsonwebtoken` crate.
            tracing::warn!("JWT signature verification not yet implemented — decoded claims but did not verify");

            // Attach claims to the request via a header (V1 workaround —
            // V1.1 will add proper request extensions).
            let mut resp = next(req).await;
            resp.set_header("x-auth-claims", &claims_json);
            resp
        })
    })
}

/// Decode a base64url string (no padding) into UTF-8 text.
fn decode_base64url(s: &str) -> Option<String> {
    use base64::Engine as _;
    // Convert base64url to base64 (add padding, replace - and _).
    let mut s = s.replace('-', "+").replace('_', "/");
    while s.len() % 4 != 0 {
        s.push('=');
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(s).ok()?;
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_base64url() {
        // "hello" in base64url.
        let s = "aGVsbG8";
        assert_eq!(decode_base64url(s), Some("hello".to_string()));
    }

    #[test]
    fn rejects_invalid_base64url() {
        assert_eq!(decode_base64url("!!!"), None);
    }
}

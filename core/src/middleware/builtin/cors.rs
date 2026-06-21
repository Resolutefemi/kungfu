//! CORS middleware.
//!
//! Defaults to a permissive-but-explicit config: the framework logs a warning
//! if `allow_origin` is set to `*` AND credentials are enabled, because that
//! combination is a spec violation that browsers reject.

use std::sync::Arc;

use crate::middleware::{Middleware, Next};
use crate::request::{Method, Request};
use crate::response::Response;

#[derive(Debug, Clone)]
pub struct CorsConfig {
    pub allow_origin: String,
    pub allow_methods: Vec<Method>,
    pub allow_headers: Vec<String>,
    pub expose_headers: Vec<String>,
    pub allow_credentials: bool,
    pub max_age_seconds: u32,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allow_origin: "*".into(),
            allow_methods: vec![
                Method::Get,
                Method::Post,
                Method::Put,
                Method::Patch,
                Method::Delete,
                Method::Options,
            ],
            allow_headers: vec![
                "authorization".into(),
                "content-type".into(),
                "x-requested-with".into(),
            ],
            expose_headers: vec![],
            allow_credentials: false,
            max_age_seconds: 600,
        }
    }
}

pub fn cors() -> Middleware {
    cors_with(CorsConfig::default())
}

pub fn cors_with(config: CorsConfig) -> Middleware {
    if config.allow_origin == "*" && config.allow_credentials {
        tracing::warn!(
            "CORS: allow_origin='*' + allow_credentials=true is rejected by browsers. \
             Specify an explicit origin instead."
        );
    }

    let allow_methods = config
        .allow_methods
        .iter()
        .map(|m| m.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let allow_headers = config.allow_headers.join(", ");
    let expose_headers = config.expose_headers.join(", ");

    Arc::new(move |req: Request, next: Next| {
        let config = config.clone();
        let allow_methods = allow_methods.clone();
        let allow_headers = allow_headers.clone();
        let expose_headers = expose_headers.clone();
        Box::pin(async move {
            // Pre-flight short-circuit.
            if req.method == Method::Options {
                let mut resp = Response::new().text("");
                resp.set_status(crate::error::StatusCode::NoContent);
                resp.set_header("access-control-allow-origin", &config.allow_origin);
                resp.set_header("access-control-allow-methods", &allow_methods);
                resp.set_header("access-control-allow-headers", &allow_headers);
                resp.set_header(
                    "access-control-max-age",
                    config.max_age_seconds.to_string(),
                );
                if config.allow_credentials {
                    resp.set_header("access-control-allow-credentials", "true");
                }
                if !expose_headers.is_empty() {
                    resp.set_header("access-control-expose-headers", &expose_headers);
                }
                return resp;
            }

            let mut resp = next(req).await;
            resp.set_header("access-control-allow-origin", &config.allow_origin);
            if config.allow_credentials {
                resp.set_header("access-control-allow-credentials", "true");
            }
            if !expose_headers.is_empty() {
                resp.set_header("access-control-expose-headers", &expose_headers);
            }
            resp
        })
    })
}

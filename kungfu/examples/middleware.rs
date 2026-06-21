//! Example: custom middleware.
//!
//! Run with: `cargo run -p kungfu --example middleware`
//!
//! Demonstrates:
//! - Writing a custom middleware that adds a response header
//! - Short-circuiting the chain (return a Response without calling next)
//! - Installing the middleware on the KungfuBuilder

use kungfu::prelude::*;
use std::sync::Arc;

fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    // A middleware that adds a custom header to every response.
    let add_request_id: kungfu::Middleware = Arc::new(|req, next| {
        Box::pin(async move {
            let request_id = req
                .header("x-request-id")
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("req-{}", std::process::id()));
            let mut resp = next(req).await;
            resp.set_header("x-request-id", request_id);
            resp
        })
    });

    // A middleware that blocks requests without an API key.
    let auth_required: kungfu::Middleware = Arc::new(|req, next| {
        Box::pin(async move {
            if req.header("x-api-key").is_none() {
                return kungfu::Response::new()
                    .status(kungfu::StatusCode::Unauthorized)
                    .json(&serde_json::json!({
                        "error": "Missing X-API-Key header"
                    }));
            }
            next(req).await
        })
    });

    let hello = get!("/hello", |_req: kungfu::Request| {
        kungfu::Response::new().json(&serde_json::json!({"message":"world"}))
    });

    let protected = get!("/protected", |_req: kungfu::Request| {
        kungfu::Response::new().json(&serde_json::json!({"secret":"data"}))
    });

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(
        Kungfu::new()
            .title("Middleware Example")
            .use_middleware(add_request_id)
            .route(hello)
            .route(protected)
            .use_middleware(auth_required)
            .run("0.0.0.0:3000"),
    )
    .unwrap();
}

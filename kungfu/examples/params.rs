//! Example: route parameters + query strings.
//!
//! Run with: `cargo run -p kungfu --example params`
//!
//! Demonstrates:
//! - `:id` path parameters
//! - `*path` wildcard parameters
//! - Query string parsing

use kungfu::prelude::*;

fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    // /users/:id → req.param("id")
    let get_user = get!("/users/:id", |req: kungfu::Request| {
        let id = req.param("id").unwrap_or("unknown").to_string();
        kungfu::Response::new().json(&serde_json::json!({
            "user_id": id,
            "name": format!("User {}", id),
        }))
    });

    // /search?q=rust&limit=10 → req.query("q"), req.query("limit")
    let search = get!("/search", |req: kungfu::Request| {
        let q = req.query("q").unwrap_or("").to_string();
        let limit: usize = req.query("limit").and_then(|s| s.parse().ok()).unwrap_or(10);
        kungfu::Response::new().json(&serde_json::json!({
            "query": q,
            "limit": limit,
            "results": [],
        }))
    });

    // /assets/*path → req.param("path") captures everything after /assets/
    let asset = get!("/assets/*path", |req: kungfu::Request| {
        let path = req.param("path").unwrap_or("").to_string();
        kungfu::Response::new().json(&serde_json::json!({
            "asset_path": path,
            "would_serve": format!("/var/www/{}", path),
        }))
    });

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(
        Kungfu::new()
            .title("Params Example")
            .route(get_user)
            .route(search)
            .route(asset)
            .run("0.0.0.0:3000"),
    )
    .unwrap();
}

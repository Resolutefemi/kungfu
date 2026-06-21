//! Example: hot reload.
//!
//! Run with: `cargo run -p kungfu --example hot_reload`
//!
//! Demonstrates:
//! - Running a Kungfu server with hot-reload enabled
//! - The file watcher fires on changes to .rs files in src/ and routes/
//! - The reload callback receives the router slot and can swap in a new router
//!
//! To test:
//! 1. Run this example
//! 2. curl http://localhost:3000/hello → returns {"message":"world"}
//! 3. Edit `kungfu/src/lib.rs` and save
//! 4. The watcher fires and the callback runs

use kungfu::prelude::*;
use kungfu::server::{HotReloadConfig, RouterSlot};
use std::sync::Arc;

fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let hello = get!("/hello", |_req: kungfu::Request| {
        kungfu::Response::new().json(&serde_json::json!({"message":"world"}))
    });

    // Build a fresh router for the reload callback.
    let build_router = || {
        let hello = get!("/hello", |_req: kungfu::Request| {
            kungfu::Response::new().json(&serde_json::json!({"message":"world (reloaded)"}))
        });
        let mut router = kungfu::Router::new();
        let _ = router.add_with_meta(
            kungfu::RouteMeta {
                path: "/hello".into(),
                method: kungfu::Method::Get,
                ..Default::default()
            },
            {
                use kungfu::__macro_support::make_handler;
                use std::sync::Arc;
                let f = |_req: kungfu::Request| {
                    kungfu::Response::new().json(&serde_json::json!({"message":"world (reloaded)"}))
                };
                make_handler(f)
            },
        );
        router
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let config = HotReloadConfig::default();

    rt.block_on(
        Kungfu::new()
            .title("Hot Reload Example")
            .route(hello)
            .run_with_hot_reload("0.0.0.0:3000", config, |router_slot: &Arc<RouterSlot>| {
                tracing::info!("Hot reload triggered — swapping router");
                kungfu::server::swap_router(router_slot, build_router());
            }),
    )
    .unwrap();
}

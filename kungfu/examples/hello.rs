//! Hello-world example using the `kungfu` idiomatic API.
//!
//! Run with:
//!   cargo run -p kungfu --example hello

use kungfu::prelude::*;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    // The `#[get]` macro expands to a register function. We pass that function
    // to `.route()` to register it on the router.
    let register_hello = get!("/hello", |_req: kungfu::Request| {
        kungfu::Response::new().json(&serde_json::json!({
            "message": "world",
            "framework": "kungfu",
            "version": kungfu::VERSION,
        }))
    });

    let register_echo = post!("/echo/:name", |req: kungfu::Request| {
        kungfu::Response::new().json(&serde_json::json!({
            "hello": req.param("name").unwrap_or("anonymous"),
            "you_sent": req.json_value().unwrap_or(serde_json::json!({})),
        }))
    });

    // Use a synchronous entry point that wraps tokio.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    rt.block_on(
        Kungfu::new()
            .title("Hello Kungfu")
            .route(register_hello)
            .route(register_echo)
            .run("0.0.0.0:3000"),
    )
    .expect("server failed");
}

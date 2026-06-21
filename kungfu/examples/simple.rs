//! Example: the simplest possible Kungfu app (no macros).
//!
//! Run with: `cargo run -p kungfu --example simple`
//!
//! Demonstrates:
//! - `handle_get` / `handle_post` — closure-based handlers (no macros)
//! - `json_get` / `json_post` — even less boilerplate for JSON endpoints
//! - Chainable `ResponseBuilder` (status, header, text, html, json, send)

use kungfu::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct CreateUser {
    email: String,
}

#[derive(Serialize)]
struct User {
    id: u64,
    email: String,
}

fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(
        Kungfu::new()
            .title("Simple API Example")
            // Simplest possible: closure with ResponseBuilder.
            .handle_get("/hello", |_req, res| res.text("world"))
            // JSON-returning closure with no body parsing.
            .json_get("/health", || serde_json::json!({"status":"ok"}))
            // JSON endpoint that parses the request body into a typed struct.
            .json_post("/users", |body: CreateUser| User {
                id: 1,
                email: body.email,
            })
            // HTML response.
            .handle_get("/", |_req, res| {
                res.html("<h1>Hello from Kungfu!</h1><p>Visit /hello or /health</p>")
            })
            // Custom status + headers.
            .handle_get("/teapot", |_req, res| {
                res.status(418).header("x-tea", "earl grey").text("I'm a teapot")
            })
            .run("0.0.0.0:3000"),
    )
    .unwrap();
}

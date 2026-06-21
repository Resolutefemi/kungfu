//! Node.js binding for the Kungfu.js framework.
//!
//! V1 approach: expose individual Rust functions (CSS compilation, server
//! start, ORM helpers) as napi functions. The HTTP server runs entirely in
//! Rust for maximum performance. JS handler bridging will be added in V1.1
//! via ThreadsafeFunction (requires careful Send/Sync handling).
//!
//! ## Available functions
//!
//! - `compileCss(classes: string): string` — compile utility classes to CSS
//! - `compileCssDir(dir: string): string` — scan directory + emit CSS
//! - `startServer(port: number): Promise<void>` — start the Rust HTTP server
//! - `version(): string` — get framework version

#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;

/// Compile a class string (e.g. "flex p-4 text-red-500") to CSS.
#[napi]
pub fn compile_css(classes: String) -> String {
    kungfu_css::compile_classes(&classes)
}

/// Scan a directory for class= / className= usage and emit a minimal CSS bundle.
#[napi]
pub fn compile_css_dir(dir: String) -> Result<String> {
    kungfu_css::compile_directory(&dir)
        .map_err(|e| Error::new(Status::GenericFailure, format!("CSS scan: {e}")))
}

/// Get the framework version.
#[napi]
pub fn version() -> String {
    kungfu_core::VERSION.to_string()
}

/// Start the Kungfu HTTP server on the given port. Returns a Promise that
/// resolves when the server stops.
///
/// The server comes with built-in routes:
///   - GET /hello → {"message":"world"}
///   - GET /docs → Swagger UI
///   - GET /openapi.json → OpenAPI 3.1 spec
///
/// V1.1 will add JS handler registration via ThreadsafeFunction.
#[napi]
pub async fn start_server(port: u16) -> Result<()> {
    let addr: std::net::SocketAddr = format!("0.0.0.0:{port}")
        .parse()
        .map_err(|e| Error::new(Status::InvalidArg, format!("invalid port: {e}")))?;

    // Build a router with a built-in hello route.
    let mut router = kungfu_core::Router::new();

    // Pre-serialised hello response (cached — fastest path).
    let hello_body = bytes::Bytes::from(
        serde_json::to_vec(&serde_json::json!({
            "message": "world",
            "framework": "kungfu",
            "version": kungfu_core::VERSION,
        }))
        .unwrap(),
    );
    let hello_for_handler = hello_body.clone();
    router.add_with_meta(
        kungfu_core::RouteMeta {
            path: "/hello".into(),
            method: kungfu_core::Method::Get,
            ..Default::default()
        },
        std::sync::Arc::new(move |_req| {
            let body = hello_for_handler.clone();
            Box::pin(async move { kungfu_core::Response::new().json_bytes(body) })
        }),
    )
    .map_err(|e| Error::new(Status::GenericFailure, format!("route: {e}")))?;

    // Install default middleware + auto docs.
    for mw in kungfu_core::default_middleware_stack().into_iter().rev() {
        router.prepend_middleware(mw);
    }
    let _ = kungfu_core::openapi::register_docs_routes(
        &mut router,
        "Kungfu API",
        kungfu_core::VERSION,
    );

    let n_cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let server = kungfu_core::Server::new(router, addr).with_acceptor_threads(n_cpus);
    server
        .serve()
        .await
        .map_err(|e| Error::new(Status::GenericFailure, format!("server: {e}")))
}

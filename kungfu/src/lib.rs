//! kungfu — the idiomatic Rust API for the Kungfu.js polyglot framework.
//!
//! ## Quickstart
//!
//! ```no_run
//! use kungfu::prelude::*;
//! use kungfu::{Request, Response};
//!
//! fn main() {
//!     let register_hello = get!("/hello", |_req: Request| {
//!         Response::new().json(&serde_json::json!({"message":"world"}))
//!     });
//!
//!     let rt = tokio::runtime::Builder::new_multi_thread()
//!         .enable_all().build().unwrap();
//!     rt.block_on(
//!         Kungfu::new()
//!             .route(register_hello)
//!             .run("0.0.0.0:3000"),
//!     ).unwrap();
//! }
//! ```

pub mod prelude;
pub mod builder;
pub mod macros;

// Re-export `__macro_support` at the crate root so `#[macro_export]` macros
// can find it via `$crate::__macro_support`.
pub use macros::__macro_support;

pub use builder::{Kungfu, KungfuBuilder};
pub use kungfu_core::{
    default_middleware_stack,
    error::{KungfuError, Result, StatusCode},
    middleware::{build_chain, Middleware, Next, NextFuture},
    middleware_builtin,
    openapi,
    request::{parse_query, Method, Request},
    response::Response,
    router::{Handler, RouteMeta, RouteResolution, Router},
    server,
    server::Server,
    version::VERSION,
};

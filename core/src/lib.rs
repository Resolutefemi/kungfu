//! kungfu-core — the engine behind the Kungfu.js polyglot framework.
//!
//! This crate is intentionally framework-only: it has no opinion on how
//! language bindings wire up to it. The Rust-native idiomatic API lives in
//! the `kungfu` crate (`kungfu/src/lib.rs`), which re-exports from here and
//! adds the `#[get]` / `#[post]` proc macros.

#![forbid(unsafe_code)]

pub mod error;
pub mod headers;
pub mod cookies;
pub mod middleware;
pub mod openapi;
pub mod request;
pub mod response;
pub mod router;
pub mod server;
pub mod version;
pub mod websocket;

pub mod middleware_builtin {
    pub use crate::middleware::builtin::cors::{cors, cors_with, CorsConfig};
    pub use crate::middleware::builtin::etag::etag;
    pub use crate::middleware::builtin::gzip::gzip;
    pub use crate::middleware::builtin::logger::logger;
    pub use crate::middleware::builtin::rate_limiter::{rate_limiter, rate_limiter_with, RateLimiterConfig};
    pub use crate::middleware::builtin::security_headers::{
        security_headers, security_headers_with, SecurityConfig,
    };
    pub use crate::middleware::builtin::serve_static::serve_static;
}

pub use error::{KungfuError, Result, StatusCode};
pub use headers::Headers;
pub use cookies::{Cookie, CookieJar, SameSite};
pub use middleware::{build_chain, Middleware, Next, NextFuture};
pub use request::{parse_query, Method, Request};
pub use response::Response;
pub use router::{Handler, RouteMeta, RouteResolution, Router};
pub use server::Server;
pub use version::VERSION;
pub use websocket::{compute_accept_key, WebSocket, WebSocketHandler, WebSocketMessage};

/// Convenience: the default secure-by-default middleware stack.
///
/// Order matters: security headers must be outermost so they apply to
/// short-circuiting responses (e.g. rate-limited 429s).
pub fn default_middleware_stack() -> Vec<Middleware> {
    vec![
        middleware_builtin::security_headers(),
        middleware_builtin::cors(),
        middleware_builtin::rate_limiter(),
        middleware_builtin::logger(),
    ]
}

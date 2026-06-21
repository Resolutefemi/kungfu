//! Built-in middleware.
//!
//! All built-in middleware is **secure-by-default**: the framework emits a
//! warning (via `tracing::warn!`) if a developer disables one. The middleware
//! here covers the spec's "always active" set:
//!   - logger
//!   - cors
//!   - security_headers
//!   - rate_limiter
//!   - serve_static (opt-in)

pub mod logger;
pub mod cors;
pub mod security_headers;
pub mod rate_limiter;
pub mod serve_static;
pub mod etag;
pub mod gzip;
pub mod validate;

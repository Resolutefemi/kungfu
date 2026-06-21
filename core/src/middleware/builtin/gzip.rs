//! Gzip compression middleware.
//!
//! Compresses response bodies with gzip when the client sends
//! `Accept-Encoding: gzip`. Skips small bodies (<256 bytes — compression
//! overhead exceeds the savings), already-compressed content types
//! (image/*, video/*, application/zip), and responses that already have
//! a `Content-Encoding` header.
//!
//! ## Example
//!
//! ```ignore
//! use kungfu::middleware_builtin::gzip;
//!
//! Kungfu::new()
//!     .use_middleware(gzip())
//!     .handle_get("/large.json", |_req, res| res.json(&big_data()))
//! ```

use std::sync::Arc;

use crate::middleware::{Middleware, Next};
use crate::request::Request;
use crate::response::Response;

/// Minimum body size to bother compressing. Below this, the gzip header
/// overhead exceeds the savings.
const MIN_COMPRESS_SIZE: usize = 256;

/// Create a gzip compression middleware.
pub fn gzip() -> Middleware {
    Arc::new(|req: Request, next: Next| {
        Box::pin(async move {
            let accepts_gzip = req
                .header("accept-encoding")
                .map(|v| v.to_ascii_lowercase().contains("gzip"))
                .unwrap_or(false);

            let mut resp = next(req).await;

            if !accepts_gzip || resp.body.len() < MIN_COMPRESS_SIZE {
                return resp;
            }

            // Skip if already encoded.
            if resp.header_value("content-encoding").is_some() {
                return resp;
            }

            // Skip already-compressed content types.
            if let Some(ct) = resp.header_value("content-type") {
                let ct = ct.to_ascii_lowercase();
                let skip = ct.starts_with("image/")
                    || ct.starts_with("video/")
                    || ct.starts_with("audio/")
                    || ct.contains("zip")
                    || ct.contains("gzip")
                    || ct.contains("compress")
                    || ct.contains("octet-stream");
                if skip {
                    return resp;
                }
            }

            // Compress.
            use flate2::write::GzEncoder;
            use std::io::Write;
            let encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
            let mut encoder = encoder;
            if let Err(e) = encoder.write_all(&resp.body) {
                tracing::warn!("gzip encode error: {e}");
                return resp;
            }
            match encoder.finish() {
                Ok(compressed) => {
                    resp.body = bytes::Bytes::from(compressed);
                    resp.set_header("content-encoding", "gzip");
                    resp.set_header("vary", "Accept-Encoding");
                }
                Err(e) => {
                    tracing::warn!("gzip finish error: {e}");
                }
            }
            resp
        })
    })
}

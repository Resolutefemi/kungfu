//! ETag middleware — adds ETag headers to responses and handles conditional GETs.
//!
//! Generates a weak ETag from the response body (xxHash-like FNV-1a hash).
//! On requests with `If-None-Match`, returns 304 Not Modified if the ETag
//! matches, saving bandwidth.
//!
//! ## Example
//!
//! ```ignore
//! use kungfu::middleware_builtin::etag;
//!
//! Kungfu::new()
//!     .use_middleware(etag())
//!     .handle_get("/large.json", |_req, res| res.json(&big_data()))
//! ```

use std::sync::Arc;

use crate::middleware::{Middleware, Next};
use crate::request::Request;

/// Create an ETag middleware.
pub fn etag() -> Middleware {
    Arc::new(|req: Request, next: Next| {
        Box::pin(async move {
            let if_none_match = req
                .header("if-none-match")
                .map(|s| s.to_string());

            let mut resp = next(req).await;

            // Skip ETag for empty bodies or responses with status >= 300.
            if resp.body.is_empty() || resp.status.as_u16() >= 300 {
                return resp;
            }

            // Compute ETag from body using FNV-1a (fast, no deps).
            let tag = fnv1a_64(&resp.body);
            let etag_value = format!("\"{tag:x}\"");
            resp.set_header("etag", &etag_value);

            // If-None-Match → 304 Not Modified.
            if let Some(inm) = &if_none_match {
                if inm == &etag_value || inm == "*" {
                    // Strip body — RFC 7232 §4.1 says 304 should not have a body.
                    resp.body = bytes::Bytes::new();
                    resp.set_status(crate::StatusCode::NotImplemented); // 304 not in our enum; use 304 via header trick
                    resp.set_status(crate::StatusCode::from(304));
                    // Remove content-type and content-length — they're meaningless for 304.
                    resp.headers.remove("content-type");
                    resp.headers.remove("content-length");
                }
            }

            resp
        })
    })
}

/// 64-bit FNV-1a hash. Fast, no dependencies, good enough for ETag generation.
fn fnv1a_64(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for &b in data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_is_deterministic() {
        assert_eq!(fnv1a_64(b"hello"), fnv1a_64(b"hello"));
        assert_ne!(fnv1a_64(b"hello"), fnv1a_64(b"world"));
    }

    #[test]
    fn fnv1a_known_values() {
        // FNV-1a 64-bit of empty string is the offset basis.
        assert_eq!(fnv1a_64(b""), 0xcbf29ce484222325);
    }
}

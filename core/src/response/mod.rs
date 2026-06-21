//! Response model.

use bytes::Bytes;
use crate::error::{KungfuError, StatusCode};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::BTreeMap;

/// Pre-serialised common error bodies. Avoids re-encoding JSON on every
/// 404/405/429 — these are surprisingly hot paths in production.
static ERROR_404_BODY: Lazy<Bytes> = Lazy::new(|| {
    Bytes::from_static(
        br#"{"error":{"code":404,"detail":null,"message":"Not Found","suggestion":null}}"#,
    )
});
static ERROR_405_BODY: Lazy<Bytes> = Lazy::new(|| {
    Bytes::from_static(
        br#"{"error":{"code":405,"detail":null,"message":"Method Not Allowed","suggestion":null}}"#,
    )
});
static ERROR_429_BODY: Lazy<Bytes> = Lazy::new(|| {
    Bytes::from_static(
        br#"{"error":{"code":429,"detail":null,"message":"Too Many Requests","suggestion":null}}"#,
    )
});

/// A response that handlers build up and the server flushes to the wire.
#[derive(Debug)]
pub struct Response {
    pub status: StatusCode,
    /// Headers — `BTreeMap` so the order is deterministic in serialised output
    /// (helps with HTTP/2 + caching layers) and case-insensitive lookups are
    /// still cheap at small N.
    pub headers: BTreeMap<String, String>,
    /// Body stored as `Bytes` — `Bytes::clone()` is an atomic Arc increment,
    /// so cached responses cost ~10ns to reuse vs. ~200ns to re-serialise.
    pub body: Bytes,
    /// Set to true by `send_file` / any path that already encoded the body.
    pub finalised: bool,
}

impl Default for Response {
    fn default() -> Self {
        let mut headers = BTreeMap::new();
        headers.insert("server".into(), crate::version::banner());
        headers.insert("x-powered-by".into(), crate::version::banner());
        Self {
            status: StatusCode::Ok,
            headers,
            body: Bytes::new(),
            finalised: false,
        }
    }
}

impl Response {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn status(mut self, code: StatusCode) -> Self {
        self.status = code;
        self
    }

    pub fn set_status(&mut self, code: StatusCode) {
        self.status = code;
    }

    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into().to_ascii_lowercase(), value.into());
        self
    }

    pub fn set_header(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.headers.insert(key.into().to_ascii_lowercase(), value.into());
    }

    pub fn header_value(&self, key: &str) -> Option<&str> {
        self.headers.get(&key.to_ascii_lowercase()).map(|s| s.as_str())
    }

    /// Set body from raw bytes. Cheap if `data` is already `Bytes`.
    pub fn send(mut self, data: impl Into<Bytes>) -> Self {
        self.body = data.into();
        self.finalised = true;
        self
    }

    /// Set body from a pre-serialised `Bytes` — the fastest path. Use this
    /// for cached responses where the JSON is computed once at startup.
    pub fn send_bytes(mut self, data: Bytes) -> Self {
        self.body = data;
        self.finalised = true;
        self
    }

    pub fn text(mut self, s: impl Into<String>) -> Self {
        let s = s.into();
        if self.header_value("content-type").is_none() {
            self.headers
                .insert("content-type".into(), "text/plain; charset=utf-8".into());
        }
        self.body = Bytes::from(s);
        self.finalised = true;
        self
    }

    pub fn html(mut self, s: impl Into<String>) -> Self {
        let s = s.into();
        self.headers
            .insert("content-type".into(), "text/html; charset=utf-8".into());
        self.body = Bytes::from(s);
        self.finalised = true;
        self
    }

    pub fn json(mut self, value: &impl Serialize) -> Self {
        #[cfg(feature = "simd")]
        let body = {
            // simd-json's serialiser produces a Vec<u8> directly.
            simd_json::to_string(value)
                .map(|s| s.into_bytes())
                .expect("response json serialisation failed")
        };
        #[cfg(not(feature = "simd"))]
        let body = serde_json::to_vec(value).expect("response json serialisation failed");
        self.headers
            .insert("content-type".into(), "application/json; charset=utf-8".into());
        self.body = Bytes::from(body);
        self.finalised = true;
        self
    }

    /// Like `json()` but takes a pre-serialised `Bytes` — the hot path for
    /// handlers that return the same JSON every time (e.g. `/health`).
    /// The `Bytes` is cloned (atomic increment) rather than re-serialised.
    pub fn json_bytes(mut self, body: Bytes) -> Self {
        self.headers
            .insert("content-type".into(), "application/json; charset=utf-8".into());
        self.body = body;
        self.finalised = true;
        self
    }

    pub fn error(mut self, err: KungfuError) -> Self {
        // Fast paths for the most common errors — use pre-serialised bodies.
        let cached: Option<Bytes> = match err.code {
            StatusCode::NotFound => Some(ERROR_404_BODY.clone()),
            StatusCode::MethodNotAllowed => Some(ERROR_405_BODY.clone()),
            StatusCode::TooManyRequests => Some(ERROR_429_BODY.clone()),
            _ => None,
        };
        self.status = err.code;
        self.headers
            .insert("content-type".into(), "application/json; charset=utf-8".into());
        self.body = cached.unwrap_or_else(|| {
            Bytes::from(serde_json::to_vec(&err.to_json()).unwrap_or_else(|_| b"{}".to_vec()))
        });
        self.finalised = true;
        self
    }

    /// HTML-escape a string before embedding it in an HTML response.
    ///
    /// XSS protection: `res.send()` does *not* auto-escape (handlers may want
    /// to send bytes), but `res.html_escape()` is the explicit safe path.
    /// We escape the OWASP-recommended set: `& < > " '`. We do NOT escape
    /// `/` because it's only meaningful inside tag-attribute contexts, not
    /// in body text — and we want the escaped output to be readable.
    pub fn html_escape(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                '\'' => out.push_str("&#x27;"),
                _ => out.push(c),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_escape_replaces_all_dangerous_chars() {
        assert_eq!(
            Response::html_escape("<script>alert('xss')</script>"),
            "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
        );
    }

    #[test]
    fn json_response_sets_content_type() {
        let resp = Response::new().json(&serde_json::json!({"ok": true}));
        assert_eq!(resp.status, StatusCode::Ok);
        assert_eq!(
            resp.header_value("content-type"),
            Some("application/json; charset=utf-8")
        );
        assert_eq!(resp.body, Bytes::from_static(br#"{"ok":true}"#));
    }

    #[test]
    fn cached_json_clones_cheaply() {
        // Pre-serialise once.
        let cached: Bytes = Bytes::from_static(br#"{"message":"world"}"#);
        // Clone is an atomic increment, not a memcpy.
        let resp1 = Response::new().json_bytes(cached.clone());
        let resp2 = Response::new().json_bytes(cached.clone());
        let resp3 = Response::new().json_bytes(cached);
        assert_eq!(resp1.body, resp2.body);
        assert_eq!(resp2.body, resp3.body);
    }

    #[test]
    fn error_404_uses_pre_serialised_body() {
        let resp = Response::new().error(KungfuError::not_found("anything"));
        // The cached 404 body doesn't include the dynamic message — that's
        // the trade-off for speed. Status code is still 404.
        assert_eq!(resp.status, StatusCode::NotFound);
        assert_eq!(resp.body, *ERROR_404_BODY);
    }
}

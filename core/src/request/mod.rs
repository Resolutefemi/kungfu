//! Request model.
//!
//! Designed to be cheap to construct and zero-copy where it matters. Headers
//! are stored as a `Vec<(String, String)>` rather than a HashMap because the
//! common case is a handful of headers per request — a linear scan is faster
//! than hashing for N < ~16.

use std::collections::HashMap;

use bytes::Bytes;

use crate::error::{KungfuError, Result, StatusCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Method {
    #[default]
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

impl Method {
    pub fn as_str(&self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
            Method::Patch => "PATCH",
            Method::Head => "HEAD",
            Method::Options => "OPTIONS",
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        match bytes {
            b"GET" => Some(Self::Get),
            b"POST" => Some(Self::Post),
            b"PUT" => Some(Self::Put),
            b"DELETE" => Some(Self::Delete),
            b"PATCH" => Some(Self::Patch),
            b"HEAD" => Some(Self::Head),
            b"OPTIONS" => Some(Self::Options),
            _ => None,
        }
    }
}

/// An incoming HTTP request.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    /// The path portion of the URL (no query string).
    pub path: String,
    /// Raw query string (without the leading `?`).
    pub query_string: String,
    /// Parsed query parameters.
    pub query: HashMap<String, String>,
    /// HTTP version, e.g. "HTTP/1.1".
    pub version: String,
    /// Headers, lowercased keys.
    pub headers: Vec<(String, String)>,
    /// Route parameters extracted by the trie router (e.g. `:id` → `"42"`).
    pub params: HashMap<String, String>,
    /// Raw body bytes — `Bytes` for cheap cloning (atomic Arc increment).
    pub body: Bytes,
    /// Client remote address (best-effort).
    pub remote_addr: Option<std::net::SocketAddr>,
}

impl Request {
    pub fn new(method: Method, path: impl Into<String>) -> Self {
        let path = path.into();
        Self {
            method,
            path,
            query_string: String::new(),
            query: HashMap::new(),
            version: "HTTP/1.1".to_string(),
            headers: Vec::new(),
            params: HashMap::new(),
            body: Bytes::new(),
            remote_addr: None,
        }
    }

    /// Look up a header (case-insensitive).
    pub fn header(&self, key: &str) -> Option<&str> {
        let key_lower = key.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(&key_lower))
            .map(|(_, v)| v.as_str())
    }

    pub fn content_type(&self) -> Option<&str> {
        self.header("content-type")
    }

    pub fn content_length(&self) -> Option<usize> {
        self.header("content-length").and_then(|v| v.parse().ok())
    }

    /// Parse the body as JSON. Returns an error that the framework can surface
    /// directly to the client (status 400 + actionable suggestion).
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T> {
        #[cfg(feature = "simd")]
        {
            // simd-json requires a mutable copy of the input.
            let mut bytes = self.body.to_vec();
            return simd_json::from_slice(&mut bytes).map_err(KungfuError::from);
        }
        #[cfg(not(feature = "simd"))]
        serde_json::from_slice(&self.body).map_err(KungfuError::from)
    }

    /// Parse the body as JSON, returning a `serde_json::Value` for handlers
    /// that don't want to commit to a concrete type up-front.
    pub fn json_value(&self) -> Result<serde_json::Value> {
        self.json()
    }

    /// Parse the body as `application/x-www-form-urlencoded`.
    pub fn form(&self) -> Result<HashMap<String, String>> {
        let s = std::str::from_utf8(&self.body).map_err(|_| {
            KungfuError::new(StatusCode::BadRequest, "Form body is not valid UTF-8")
        })?;
        let mut map = HashMap::new();
        for pair in s.split('&') {
            if pair.is_empty() {
                continue;
            }
            let mut iter = pair.splitn(2, '=');
            let key = iter.next().unwrap_or_default();
            let val = iter.next().unwrap_or_default();
            map.insert(
                percent_decode(key),
                percent_decode(val),
            );
        }
        Ok(map)
    }

    /// Read a route parameter set by the router.
    pub fn param(&self, key: &str) -> Option<&str> {
        self.params.get(key).map(|s| s.as_str())
    }

    /// Read a query parameter.
    pub fn query(&self, key: &str) -> Option<&str> {
        self.query.get(key).map(|s| s.as_str())
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
                out.push(b);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_default()
}

/// Parse a query string into a `HashMap`.
pub fn parse_query(qs: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if qs.is_empty() {
        return map;
    }
    for pair in qs.split('&') {
        let mut iter = pair.splitn(2, '=');
        let key = iter.next().unwrap_or_default().to_string();
        let val = iter.next().unwrap_or_default().to_string();
        map.insert(percent_decode(&key), percent_decode(&val));
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_form_body() {
        let req = Request {
            method: Method::Post,
            path: "/".into(),
            query_string: String::new(),
            query: HashMap::new(),
            version: "HTTP/1.1".into(),
            headers: vec![("content-type".into(), "application/x-www-form-urlencoded".into())],
            params: HashMap::new(),
            body: Bytes::from_static(b"name=alice&email=alice%40example.com"),
            remote_addr: None,
        };
        let form = req.form().unwrap();
        assert_eq!(form.get("name"), Some(&"alice".to_string()));
        assert_eq!(form.get("email"), Some(&"alice@example.com".to_string()));
    }

    #[test]
    fn parses_query_string() {
        let q = parse_query("a=1&b=2&c=hello%20world");
        assert_eq!(q.get("a"), Some(&"1".to_string()));
        assert_eq!(q.get("b"), Some(&"2".to_string()));
        assert_eq!(q.get("c"), Some(&"hello world".to_string()));
    }
}

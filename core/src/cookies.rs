//! Cookie support.
//!
//! Provides `Cookie`, `CookieJar`, and helpers for parsing the `Cookie`
//! request header and serialising the `Set-Cookie` response header.
//!
//! ## Example
//!
//! ```ignore
//! use kungfu::cookies::CookieJar;
//!
//! Kungfu::new()
//!     .handle_get("/login", |_req, res| {
//!         let mut jar = CookieJar::new();
//!         jar.set(Cookie::new("session_id", "abc123").path("/").http_only(true).max_age(3600));
//!         res.header("set-cookie", &jar.to_set_cookie_header())
//!            .text("logged in")
//!     })
//!     .handle_get("/dashboard", |req, res| {
//!         let jar = CookieJar::from_request(&req);
//!         match jar.get("session_id") {
//!             Some(_) => res.text("welcome back"),
//!             None => res.status(401).text("not logged in"),
//!         }
//!     })
//! ```

use std::collections::HashMap;

use crate::request::Request;
use crate::response::Response;

/// A single HTTP cookie.
#[derive(Debug, Clone)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub path: Option<String>,
    pub domain: Option<String>,
    pub max_age: Option<u64>, // seconds
    pub http_only: bool,
    pub secure: bool,
    pub same_site: SameSite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSite {
    Strict,
    Lax,
    None,
}

impl Default for SameSite {
    fn default() -> Self {
        SameSite::Lax
    }
}

impl Cookie {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            path: None,
            domain: None,
            max_age: None,
            http_only: false,
            secure: false,
            same_site: SameSite::default(),
        }
    }

    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    pub fn max_age(mut self, seconds: u64) -> Self {
        self.max_age = Some(seconds);
        self
    }

    pub fn http_only(mut self) -> Self {
        self.http_only = true;
        self
    }

    pub fn secure(mut self) -> Self {
        self.secure = true;
        self
    }

    pub fn same_site_strict(mut self) -> Self {
        self.same_site = SameSite::Strict;
        self
    }

    pub fn same_site_lax(mut self) -> Self {
        self.same_site = SameSite::Lax;
        self
    }

    pub fn same_site_none(mut self) -> Self {
        self.same_site = SameSite::None;
        self
    }

    /// Serialise this cookie to its `Set-Cookie` value.
    pub fn to_set_cookie_string(&self) -> String {
        let mut s = format!("{}={}", self.name, self.value);
        if let Some(p) = &self.path {
            s.push_str(&format!("; Path={p}"));
        }
        if let Some(d) = &self.domain {
            s.push_str(&format!("; Domain={d}"));
        }
        if let Some(a) = self.max_age {
            s.push_str(&format!("; Max-Age={a}"));
        }
        if self.http_only {
            s.push_str("; HttpOnly");
        }
        if self.secure {
            s.push_str("; Secure");
        }
        match self.same_site {
            SameSite::Strict => s.push_str("; SameSite=Strict"),
            SameSite::Lax => s.push_str("; SameSite=Lax"),
            SameSite::None => s.push_str("; SameSite=None"),
        }
        s
    }
}

/// A jar of cookies for a single request/response cycle.
#[derive(Debug, Clone, Default)]
pub struct CookieJar {
    cookies: HashMap<String, Cookie>,
}

impl CookieJar {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse cookies from the request's `Cookie` header.
    pub fn from_request(req: &Request) -> Self {
        let mut jar = Self::new();
        if let Some(header) = req.header("cookie") {
            for pair in header.split(';') {
                let pair = pair.trim();
                if let Some(idx) = pair.find('=') {
                    let name = pair[..idx].trim().to_string();
                    let value = pair[idx + 1..].trim().to_string();
                    jar.cookies
                        .insert(name.clone(), Cookie::new(name, value));
                }
            }
        }
        jar
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.cookies.get(name).map(|c| c.value.as_str())
    }

    pub fn set(&mut self, cookie: Cookie) {
        self.cookies.insert(cookie.name.clone(), cookie);
    }

    pub fn remove(&mut self, name: &str) -> Option<Cookie> {
        self.cookies.remove(name)
    }

    pub fn len(&self) -> usize {
        self.cookies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }

    /// Serialise all cookies into the value for the `Set-Cookie` header.
    /// Multiple cookies are joined with `, ` (per RFC 6265 §5.4).
    pub fn to_set_cookie_header(&self) -> String {
        self.cookies
            .values()
            .map(|c| c.to_set_cookie_string())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Apply this jar's cookies to a response by setting the `Set-Cookie` header.
    pub fn apply_to_response(&self, resp: &mut Response) {
        if !self.cookies.is_empty() {
            resp.set_header("set-cookie", self.to_set_cookie_header());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::{Method, Request};

    #[test]
    fn parses_request_cookies() {
        let mut req = Request::new(Method::Get, "/");
        req.headers
            .push(("cookie".into(), "session=abc123; theme=dark".into()));
        let jar = CookieJar::from_request(&req);
        assert_eq!(jar.get("session"), Some("abc123"));
        assert_eq!(jar.get("theme"), Some("dark"));
        assert_eq!(jar.len(), 2);
    }

    #[test]
    fn serialises_cookie_with_all_attributes() {
        let c = Cookie::new("session", "abc123")
            .path("/")
            .domain("example.com")
            .max_age(3600)
            .http_only()
            .secure()
            .same_site_strict();
        let s = c.to_set_cookie_string();
        assert!(s.contains("session=abc123"));
        assert!(s.contains("Path=/"));
        assert!(s.contains("Domain=example.com"));
        assert!(s.contains("Max-Age=3600"));
        assert!(s.contains("HttpOnly"));
        assert!(s.contains("Secure"));
        assert!(s.contains("SameSite=Strict"));
    }

    #[test]
    fn jar_serialises_multiple_cookies() {
        let mut jar = CookieJar::new();
        jar.set(Cookie::new("a", "1").path("/"));
        jar.set(Cookie::new("b", "2").path("/"));
        let s = jar.to_set_cookie_header();
        assert!(s.contains("a=1"));
        assert!(s.contains("b=2"));
        // Multiple cookies are comma-separated in the header.
        assert!(s.contains(", "));
    }

    #[test]
    fn remove_cookie() {
        let mut jar = CookieJar::new();
        jar.set(Cookie::new("a", "1"));
        assert!(jar.remove("a").is_some());
        assert!(jar.get("a").is_none());
        assert!(jar.remove("a").is_none());
    }
}

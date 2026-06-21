//! Security headers — applied to every response, always.
//!
//! Implements the spec's "always-on" set:
//!   - Strict-Transport-Security
//!   - X-Content-Type-Options: nosniff
//!   - X-Frame-Options: DENY
//!   - Content-Security-Policy (strict)
//!   - Referrer-Policy
//!   - Permissions-Policy

use std::sync::Arc;

use crate::middleware::{Middleware, Next};
use crate::request::Request;

#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub hsts_max_age_seconds: u64,
    pub hsts_include_subdomains: bool,
    pub hsts_preload: bool,
    /// CSP applied to all responses. Strict by default.
    pub content_security_policy: String,
    pub frame_options: String,
    pub referrer_policy: String,
    pub permissions_policy: String,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            hsts_max_age_seconds: 63072000, // 2 years
            hsts_include_subdomains: true,
            hsts_preload: true,
            // Strict default: only allow same-origin resources, no inline scripts.
            content_security_policy:
                "default-src 'self'; base-uri 'self'; frame-ancestors 'none'; \
                 object-src 'none'; upgrade-insecure-requests"
                    .into(),
            frame_options: "DENY".into(),
            referrer_policy: "strict-origin-when-cross-origin".into(),
            // Lock down powerful APIs by default.
            permissions_policy: "geolocation=(), microphone=(), camera=(), payment=()".into(),
        }
    }
}

pub fn security_headers() -> Middleware {
    security_headers_with(SecurityConfig::default())
}

pub fn security_headers_with(config: SecurityConfig) -> Middleware {
    let hsts_value = format!(
        "max-age={};{}{}",
        config.hsts_max_age_seconds,
        if config.hsts_include_subdomains {
            " includeSubDomains;"
        } else {
            ""
        },
        if config.hsts_preload { " preload" } else { "" }
    );

    Arc::new(move |_req: Request, next: Next| {
        let hsts_value = hsts_value.clone();
        let csp = config.content_security_policy.clone();
        let frame = config.frame_options.clone();
        let referrer = config.referrer_policy.clone();
        let perms = config.permissions_policy.clone();
        Box::pin(async move {
            let mut resp = next(_req).await;
            resp.set_header("strict-transport-security", &hsts_value);
            resp.set_header("x-content-type-options", "nosniff");
            resp.set_header("x-frame-options", &frame);
            resp.set_header("content-security-policy", &csp);
            resp.set_header("referrer-policy", &referrer);
            resp.set_header("permissions-policy", &perms);
            resp
        })
    })
}

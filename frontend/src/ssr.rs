//! Server-side rendering entry point.
//!
//! Wraps a `.kungfu` file's `data()` + `template()` calls (executed in a
//! JS runtime) and produces a complete HTML page with the live-reload
//! script injected.

use crate::parser::KungfuFile;

/// Context for a single SSR render.
pub struct SsrContext {
    /// The request URL (used by `data()`).
    pub url: String,
    /// Request headers, serialised as a JSON object.
    pub headers: serde_json::Value,
    /// Whether to inject the live-reload script (dev mode only).
    pub inject_livereload: bool,
}

impl Default for SsrContext {
    fn default() -> Self {
        Self {
            url: "/".into(),
            headers: serde_json::json!({}),
            inject_livereload: true,
        }
    }
}

/// Render a `.kungfu` file into a complete HTML page.
///
/// `rendered_template` is the HTML string returned by the file's
/// `template()` function (the JS runtime executes this — we don't do it
/// from Rust). `data_json` is the JSON-serialised result of `data()`,
/// embedded into the page as `__KUNGFU_DATA__` for client-side hydration.
pub fn render_page(file: &KungfuFile, ctx: &SsrContext, rendered_template: &str, data_json: &serde_json::Value) -> String {
    let mut html = String::with_capacity(2048);
    html.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n");
    html.push_str("  <meta charset=\"utf-8\">\n");
    html.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str(&format!("  <title>Kungfu — {}</title>\n", file.route_path));
    html.push_str("  <link rel=\"stylesheet\" href=\"/kungfu.css\">\n");
    html.push_str("</head>\n<body>\n");
    html.push_str("  <div id=\"app\">\n");
    html.push_str("    ");
    html.push_str(rendered_template);
    html.push_str("\n  </div>\n");

    if let Some(static_html) = &file.static_html {
        html.push_str("  ");
        html.push_str(static_html);
        html.push('\n');
    }

    // Embed the SSR data for client-side hydration.
    html.push_str("  <script>window.__KUNGFU_DATA__ = ");
    html.push_str(&serde_json::to_string(data_json).unwrap_or_else(|_| "{}".into()));
    html.push_str(";</script>\n");

    if ctx.inject_livereload {
        html.push_str("  <script src=\"/__kungfu_livereload.js\"></script>\n");
    }

    html.push_str("</body>\n</html>\n");
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_page_with_data_and_template() {
        let file = crate::parser::parse_kungfu_file(
            "export function data() { return {}; }\nexport function template() { return '<h1>hi</h1>'; }",
            "src/pages/index.kungfu",
        )
        .unwrap();
        let ctx = SsrContext::default();
        let html = render_page(&file, &ctx, "<h1>hi</h1>", &serde_json::json!({"user":"Bruce"}));
        assert!(html.contains("<!doctype html>"));
        assert!(html.contains("<h1>hi</h1>"));
        assert!(html.contains("window.__KUNGFU_DATA__"));
        assert!(html.contains("\"user\":\"Bruce\""));
        assert!(html.contains("/__kungfu_livereload.js"));
    }

    #[test]
    fn render_omits_livereload_when_disabled() {
        let file = crate::parser::parse_kungfu_file(
            "export function data() { return {}; }\nexport function template() { return ''; }",
            "src/pages/index.kungfu",
        )
        .unwrap();
        let ctx = SsrContext {
            inject_livereload: false,
            ..Default::default()
        };
        let html = render_page(&file, &ctx, "", &serde_json::json!({}));
        assert!(!html.contains("livereload"));
    }
}

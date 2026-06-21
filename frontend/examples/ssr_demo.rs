//! Example: kungfu-frontend — parse a .kungfu file and render an SSR page.
//!
//! Run with: `cargo run -p kungfu-frontend --example ssr_demo`
//!
//! Demonstrates:
//! - Parsing a .kungfu file (data() + template() exports + optional static HTML)
//! - Deriving a route path from the file path
//! - Rendering a complete HTML page with livereload script + hydration data

use kungfu_frontend::{parse_kungfu_file, render_page, SsrContext};
use serde_json::json;

const KUNGFU_FILE: &str = r#"
export async function data(req) {
  return { user: { name: 'Bruce', role: 'master' } };
}

export function template({ user }) {
  return `<div class="flex p-4 text-xl">Hello, ${user.name}! You are a ${user.role}.</div>`;
}
---
<footer class="text-center p-4 text-gray-500">© 2026 Kungfu.js</footer>
"#;

fn main() {
    let file = parse_kungfu_file(KUNGFU_FILE, "src/pages/index.kungfu").unwrap();
    println!("--- Parsed .kungfu file ---");
    println!("Route path: {}", file.route_path);
    println!("Code length: {} chars", file.code.len());
    println!("Has static HTML: {}", file.static_html.is_some());

    // Simulate calling data() and template() (in a real app, a JS runtime does this).
    let data = json!({
        "user": { "name": "Bruce", "role": "master" }
    });
    let rendered_template = r#"<div class="flex p-4 text-xl">Hello, Bruce! You are a master.</div>"#;

    // Render the complete HTML page.
    let ctx = SsrContext {
        url: "/".into(),
        headers: json!({}),
        inject_livereload: true,
    };
    let html = render_page(&file, &ctx, rendered_template, &data);

    println!("\n--- Rendered HTML page ---");
    println!("{}", html);

    // Demonstrate route path derivation.
    let paths = [
        "src/pages/index.kungfu",
        "src/pages/users/index.kungfu",
        "src/pages/users/[id].kungfu",
        "src/pages/blog/[slug]/index.kungfu",
        "src/pages/assets/[...path].kungfu",
    ];
    println!("\n--- Route path derivation ---");
    for path in &paths {
        let file = parse_kungfu_file(
            "export function data() { return {}; }\nexport function template() { return ''; }",
            path,
        )
        .unwrap();
        println!("  {} → {}", path, file.route_path);
    }
}

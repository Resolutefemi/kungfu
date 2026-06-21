//! `.kungfu` file parser.
//!
//! A `.kungfu` file has two sections separated by a front-matter delimiter
//! `---`. The top section is TypeScript/JavaScript that exports `data` and
//! `template` functions. The bottom section (after `---`) is optional
//! static HTML/CSS that's prepended to the rendered template.
//!
//! In V1 we don't actually execute the TypeScript — we just parse the file
//! structure and leave execution to a JavaScript runtime (Deno/Node).
//! The Rust side is responsible for:
//!   - Splitting the file into code + static parts
//!   - Validating that `data` and `template` exports exist
//!   - Generating the wrapping HTML and injecting the live-reload script

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("missing data export")]
    MissingDataExport,
    #[error("missing template export")]
    MissingTemplateExport,
    #[error("syntax error: {0}")]
    Syntax(String),
}

#[derive(Debug, Clone)]
pub struct KungfuFile {
    /// The TypeScript code body (everything before `---`).
    pub code: String,
    /// Static HTML appended to the rendered template (everything after `---`).
    pub static_html: Option<String>,
    /// File path (without extension) used to derive the route.
    /// e.g. `src/pages/users/[id].kungfu` → `/users/:id`
    pub route_path: String,
}

/// Parse a `.kungfu` file's content.
pub fn parse_kungfu_file(content: &str, source_path: &str) -> Result<KungfuFile, ParseError> {
    let (code, static_html) = match content.find("\n---\n") {
        Some(idx) => {
            let code = content[..idx].trim().to_string();
            let rest = content[idx + 5..].trim().to_string();
            (code, if rest.is_empty() { None } else { Some(rest) })
        }
        None => (content.trim().to_string(), None),
    };

    // Validate that the code exports `data` and `template`.
    if !code.contains("export") {
        return Err(ParseError::Syntax(
            "no exports found — `.kungfu` files must export `data` and `template`".into(),
        ));
    }
    let has_data = code.contains("export async function data") || code.contains("export function data");
    let has_template = code.contains("export function template");
    if !has_data {
        return Err(ParseError::MissingDataExport);
    }
    if !has_template {
        return Err(ParseError::MissingTemplateExport);
    }

    let route_path = derive_route_path(source_path);

    Ok(KungfuFile {
        code,
        static_html,
        route_path,
    })
}

/// Convert a file path like `src/pages/users/[id].kungfu` into a route
/// path like `/users/:id`.
fn derive_route_path(source_path: &str) -> String {
    let path = source_path
        .trim_start_matches("./")
        .trim_start_matches("src/pages/")
        .trim_start_matches("pages/")
        .trim_end_matches(".kungfu");

    // `index` → `/`; `users/index` → `/users`
    let path = if path == "index" {
        ""
    } else if let Some(stripped) = path.strip_suffix("/index") {
        stripped
    } else {
        path
    };

    let mut out = String::with_capacity(path.len() + 1);
    out.push('/');
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '[' {
            // Read until ']' and emit `:name`.
            out.push(':');
            for c in chars.by_ref() {
                if c == ']' {
                    break;
                }
                out.push(c);
            }
        } else {
            out.push(c);
        }
    }
    if out.ends_with('/') && out.len() > 1 {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_kungfu_file() {
        let content = r#"
export async function data(req) {
  return { user: { name: 'Bruce' } };
}

export function template({ user }) {
  return `<div>Hello, ${user.name}!</div>`;
}
"#;
        let file = parse_kungfu_file(content, "src/pages/index.kungfu").unwrap();
        assert!(file.code.contains("export async function data"));
        assert!(file.code.contains("export function template"));
        assert!(file.static_html.is_none());
        assert_eq!(file.route_path, "/");
    }

    #[test]
    fn parses_kungfu_file_with_static_html() {
        let content = r#"
export async function data() { return {}; }
export function template() { return '<main>hi</main>'; }
---
<footer>© 2026</footer>
"#;
        let file = parse_kungfu_file(content, "src/pages/index.kungfu").unwrap();
        assert!(file.static_html.is_some());
        assert!(file.static_html.as_ref().unwrap().contains("footer"));
    }

    #[test]
    fn derives_route_path_with_params() {
        let file = parse_kungfu_file(
            "export function data() { return {}; }\nexport function template() { return ''; }",
            "src/pages/users/[id].kungfu",
        )
        .unwrap();
        assert_eq!(file.route_path, "/users/:id");
    }

    #[test]
    fn errors_on_missing_data_export() {
        let content = "export function template() { return ''; }";
        let result = parse_kungfu_file(content, "src/pages/index.kungfu");
        assert!(matches!(result, Err(ParseError::MissingDataExport)));
    }
}

//! Auto-configuration: detect `public/` directory, wire livereload, emit types.
//!
//! When you call `Kungfu::new().auto()`, the framework:
//! 1. Serves files from `public/` if the directory exists.
//! 2. Injects the livereload script into HTML responses in dev mode.
//! 3. Emits `routes.d.ts` on every route registration.
//! 4. Registers `.kungfu` files from `src/pages/` as routes.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;

use crate::router::{Router, RouteMeta};
use crate::{Method, Middleware};

/// Auto-detection configuration.
#[derive(Debug, Clone)]
pub struct AutoConfig {
    /// Serve files from this directory if it exists (default: `public`).
    pub public_dir: PathBuf,
    /// Register .kungfu files from this directory (default: `src/pages`).
    pub pages_dir: PathBuf,
    /// Emit TypeScript types to this file (default: `frontend/routes.d.ts`).
    pub types_path: PathBuf,
    /// Inject livereload script into HTML responses.
    pub livereload: bool,
}

impl Default for AutoConfig {
    fn default() -> Self {
        Self {
            public_dir: PathBuf::from("public"),
            pages_dir: PathBuf::from("src/pages"),
            types_path: PathBuf::from("frontend/routes.d.ts"),
            livereload: true,
        }
    }
}

/// Auto-configure the router with sensible defaults.
///
/// Call this after registering all routes but before `run()`.
pub fn auto_configure(router: &mut Router, config: &AutoConfig) {
    // 1. Serve static files from public/ if it exists.
    if config.public_dir.exists() {
        let mw = crate::middleware_builtin::serve_static(&config.public_dir);
        router.use_middleware(mw);
        tracing::info!("auto: serving static files from {}", config.public_dir.display());
    }

    // 2. Register .kungfu pages if src/pages/ exists.
    if config.pages_dir.exists() {
        match register_kungfu_pages(router, &config.pages_dir) {
            Ok(count) if count > 0 => {
                tracing::info!("auto: registered {count} .kungfu pages from {}", config.pages_dir.display());
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("auto: failed to register pages: {e}");
            }
        }
    }

    // 3. Emit routes.d.ts.
    let routes: Vec<RouteMeta> = router.routes();
    if !routes.is_empty() {
        let ts = generate_types(&routes);
        if let Some(parent) = config.types_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&config.types_path, &ts) {
            Ok(_) => {
                tracing::info!("auto: emitted TypeScript types to {}", config.types_path.display());
            }
            Err(e) => {
                tracing::warn!("auto: failed to emit types: {e}");
            }
        }
    }

    // 4. Livereload is handled by the DevMode controller — just log.
    if config.livereload {
        tracing::info!("auto: livereload enabled (inject script in dev mode)");
    }
}

/// Register .kungfu files from a directory as routes.
fn register_kungfu_pages(router: &mut Router, pages_dir: &Path) -> std::io::Result<usize> {
    // Delegate to the frontend crate's file_routing module.
    // Since we can't depend on kungfu-frontend from kungfu-core, we do a
    // simplified version here: walk the directory, parse .kungfu files,
    // register placeholder routes. The real SSR happens via the frontend crate.
    let mut count = 0;
    for entry in walkdir::WalkDir::new(pages_dir).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("kungfu") {
            continue;
        }

        let rel = match path.strip_prefix(pages_dir) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let route_path = derive_route_path(rel);

        let file_path = path.to_path_buf();
        let handler: crate::Handler = Arc::new(move |_req| {
            let file_path = file_path.clone();
            Box::pin(async move {
                crate::Response::new().html(format!(
                    "<!-- Kungfu SSR: {} -->\n<p>This page would be rendered via the SSR executor.</p>",
                    file_path.display()
                ))
            })
        });

        let _ = router.add_with_meta(
            RouteMeta {
                path: route_path.clone(),
                method: Method::Get,
                summary: Some(format!("SSR page: {}", route_path)),
                tags: vec!["pages".into()],
                ..Default::default()
            },
            handler,
        );
        count += 1;
    }
    Ok(count)
}

/// Convert a file path like `users/[id].kungfu` into `/users/:id`.
fn derive_route_path(rel: &Path) -> String {
    let s = rel.to_string_lossy().replace('\\', "/");
    let path = s.trim_end_matches(".kungfu");

    let mut out = String::from("/");
    for (i, seg) in path.split('/').enumerate() {
        if seg.is_empty() {
            continue;
        }
        if i > 0 || !out.ends_with('/') {
            if !out.ends_with('/') {
                out.push('/');
            }
        }
        // Check [...name] (wildcard) before [name] (param).
        if let Some(name) = seg.strip_prefix("[...").and_then(|s| s.strip_suffix(']')) {
            out.push('*');
            out.push_str(name);
        } else if let Some(name) = seg.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            out.push(':');
            out.push_str(name);
        } else {
            out.push_str(seg);
        }
    }

    // Handle index.kungfu → /
    if out.ends_with("/index") {
        out = out.trim_end_matches("/index").to_string();
    }
    if out.is_empty() {
        out = "/".to_string();
    }
    if out != "/" {
        out = out.trim_end_matches('/').to_string();
    }
    out
}

/// Generate TypeScript route types (simplified version — the full impl is in kungfu-frontend).
fn generate_types(routes: &[RouteMeta]) -> String {
    let mut out = String::from("// Auto-generated by Kungfu auto-configure. Do not edit.\n");
    out.push_str("declare namespace KungfuRoutes {\n");

    for meta in routes {
        let name = derive_interface_name(meta);
        out.push_str(&format!("  interface {} {{\n", name));
        out.push_str(&format!("    path: '{}';\n", meta.path));
        out.push_str(&format!("    method: '{}';\n", meta.method.as_str()));
        out.push_str("    response: unknown;\n");
        out.push_str("  }\n");
    }

    out.push_str("}\n");
    out
}

fn derive_interface_name(meta: &RouteMeta) -> String {
    let method = match meta.method {
        Method::Get => "Get",
        Method::Post => "Post",
        Method::Put => "Put",
        Method::Delete => "Delete",
        Method::Patch => "Patch",
        _ => "Other",
    };
    let body: String = meta
        .path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|seg| {
            if let Some(name) = seg.strip_prefix(':') {
                format!("By{}", capitalize(name))
            } else if let Some(name) = seg.strip_prefix('*') {
                format!("Star{}", capitalize(name))
            } else {
                capitalize(seg)
            }
        })
        .collect();
    format!("{}{}", method, body)
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_route_paths() {
        assert_eq!(derive_route_path(std::path::Path::new("index.kungfu")), "/");
        assert_eq!(derive_route_path(std::path::Path::new("about.kungfu")), "/about");
        assert_eq!(derive_route_path(std::path::Path::new("users/[id].kungfu")), "/users/:id");
        assert_eq!(derive_route_path(std::path::Path::new("assets/[...path].kungfu")), "/assets/*path");
    }

    #[test]
    fn generates_types() {
        let routes = vec![RouteMeta {
            path: "/hello".into(),
            method: Method::Get,
            ..Default::default()
        }];
        let ts = generate_types(&routes);
        assert!(ts.contains("interface GetHello"));
        assert!(ts.contains("path: '/hello'"));
    }
}

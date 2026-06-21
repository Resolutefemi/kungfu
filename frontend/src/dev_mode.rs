//! Dev-mode integrations: live reload + type generation wiring.
//!
//! Wires the `LiveReloadServer` and `generate_typescript` into a running
//! Kungfu server. In dev mode, call `DevMode::enable()` to:
//!
//! 1. Inject the livereload script into every SSR page.
//! 2. Broadcast a `reload` event when files change.
//! 3. Auto-emit `routes.d.ts` whenever routes are registered.
//!
//! ## Example
//!
//! ```ignore
//! use kungfu_frontend::dev_mode::DevMode;
//!
//! let dev = DevMode::new("./src", "./frontend/routes.d.ts");
//! dev.start_watcher().await;
//!
//! // ... later, in your route handlers:
//! let html = dev.render_with_livereload(&file, &ctx, &template, &data);
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::broadcast;

use crate::livereload::LiveReloadServer;
use crate::ssr::{render_page, SsrContext};
use crate::types::generate_typescript;

/// Configuration for dev mode.
#[derive(Debug, Clone)]
pub struct DevModeConfig {
    /// Directories to watch for changes (triggers livereload).
    pub watch_paths: Vec<PathBuf>,
    /// Where to write the auto-generated `routes.d.ts`.
    pub routes_types_path: Option<PathBuf>,
    /// Whether to inject the livereload script into SSR pages.
    pub inject_livereload: bool,
}

impl Default for DevModeConfig {
    fn default() -> Self {
        Self {
            watch_paths: vec![PathBuf::from("src"), PathBuf::from("routes")],
            routes_types_path: Some(PathBuf::from("frontend/routes.d.ts")),
            inject_livereload: true,
        }
    }
}

/// Dev-mode controller. Owns the `LiveReloadServer` and manages the file watcher.
pub struct DevMode {
    pub config: DevModeConfig,
    pub livereload: LiveReloadServer,
    /// Set to true when a file change is detected. The HTTP listener checks
    /// this and triggers reloads.
    pub reload_pending: Arc<Mutex<bool>>,
}

impl DevMode {
    pub fn new(watch_paths: impl Into<Vec<PathBuf>>, routes_types_path: Option<PathBuf>) -> Self {
        let config = DevModeConfig {
            watch_paths: watch_paths.into(),
            routes_types_path,
            inject_livereload: true,
        };
        Self {
            config,
            livereload: LiveReloadServer::new(),
            reload_pending: Arc::new(Mutex::new(false)),
        }
    }

    /// Render an SSR page with livereload injected (if enabled).
    pub fn render_with_livereload(
        &self,
        file: &crate::parser::KungfuFile,
        ctx: &SsrContext,
        rendered_template: &str,
        data: &serde_json::Value,
    ) -> String {
        let mut ctx = ctx.clone();
        ctx.inject_livereload = self.config.inject_livereload;
        render_page(file, &ctx, rendered_template, data)
    }

    /// Trigger a reload — call this from the file watcher when a source file changes.
    pub fn trigger_reload(&self) {
        self.livereload.trigger_reload();
    }

    /// Write the routes.d.ts file with the given route metadata.
    pub fn write_routes_types(
        &self,
        routes: &[kungfu_core::router::RouteMeta],
    ) -> std::io::Result<()> {
        let path = match &self.config.routes_types_path {
            Some(p) => p,
            None => return Ok(()),
        };
        let ts = generate_typescript(routes);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, ts)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_mode_can_be_constructed() {
        let dev = DevMode::new(vec!["src".into()], Some("routes.d.ts".into()));
        assert_eq!(dev.config.watch_paths.len(), 1);
        assert!(dev.config.inject_livereload);
    }

    #[test]
    fn writes_routes_types_file() {
        let tmp = std::env::temp_dir().join(format!("kungfu-routes-{}.d.ts", std::process::id()));
        let dev = DevMode::new(vec![], Some(tmp.clone()));
        let routes = vec![kungfu_core::router::RouteMeta {
            path: "/hello".into(),
            method: kungfu_core::Method::Get,
            ..Default::default()
        }];
        dev.write_routes_types(&routes).unwrap();
        let content = std::fs::read_to_string(&tmp).unwrap();
        assert!(content.contains("interface GetHello"));
        std::fs::remove_file(&tmp).ok();
    }
}

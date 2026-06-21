//! Plugin system for Kungfu.
//!
//! A plugin is a boxed trait object that can register middleware, routes,
//! and startup hooks. Plugins are loaded at server startup and run before
//! the server begins accepting connections.
//!
//! ## Example
//!
//! ```ignore
//! use kungfu::plugin::{Plugin, PluginContext};
//!
//! struct RequestIdPlugin;
//!
//! #[async_trait::async_trait]
//! impl Plugin for RequestIdPlugin {
//!     fn name(&self) -> &str { "request-id" }
//!
//!     async fn register(&self, ctx: &mut PluginContext) {
//!         ctx.use_middleware(Arc::new(|req, next| {
//!             Box::pin(async move {
//!                 let mut resp = next(req).await;
//!                 resp.set_header("x-request-id", uuid::Uuid::new_v4().to_string());
//!                 resp
//!             })
//!         }));
//!     }
//! }
//! ```

use std::sync::Arc;

use crate::{Middleware, Router};

/// Context passed to `Plugin::register`. Plugins can use it to add
/// middleware, routes, or run startup logic.
pub struct PluginContext {
    pub router: &'static mut Router,
}

impl PluginContext {
    /// Register a middleware on the router.
    pub fn use_middleware(&mut self, mw: Middleware) {
        // V1: this is a placeholder — actually wiring to the router requires
        // a lifetime dance that's not worth it for the scaffold.
        let _ = mw;
    }
}

/// A Kungfu plugin. Implement this trait and register your plugin via
/// `KungfuBuilder::plugin`.
#[async_trait::async_trait]
pub trait Plugin: Send + Sync {
    /// The plugin's name (used for logging + diagnostics).
    fn name(&self) -> &str;

    /// Called at server startup. Plugins can register middleware, routes,
    /// or run any other setup logic.
    async fn register(&self, _ctx: &mut PluginContext) {}

    /// Called when the server is shutting down. Optional cleanup.
    async fn shutdown(&self) {}
}

/// A collection of plugins.
pub struct PluginManager {
    plugins: Vec<Arc<dyn Plugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self { plugins: Vec::new() }
    }

    pub fn add(&mut self, plugin: Arc<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    pub async fn register_all(&self, ctx: &mut PluginContext) {
        for plugin in &self.plugins {
            tracing::info!("loading plugin: {}", plugin.name());
            plugin.register(ctx).await;
        }
    }

    pub async fn shutdown_all(&self) {
        for plugin in &self.plugins {
            plugin.shutdown().await;
        }
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

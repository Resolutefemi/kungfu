//! Path-parameterised trie router.
//!
//! Each segment of the path is a node. A node has up to three children:
//!   - a static child map (segment literal → child)
//!   - one parameter child (`:name`)
//!   - one wildcard child (`*rest`)
//!
//! Lookup walks the trie segment-by-segment. Insertion is O(path depth);
//! lookup is O(path depth). For 16-core throughput this is the fastest
//! commonly-used routing structure (the alternative — a radix tree like
//! Actix's — has slightly better cache behaviour for very large route tables
//! but the same big-O and a more complex implementation).

use std::collections::HashMap;

use crate::error::{KungfuError, Result, StatusCode};
use crate::request::{Method, Request};
use crate::response::Response;

/// A handler is any async function taking `Request` and returning `Response`.
pub type Handler = std::sync::Arc<
    dyn Fn(Request) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
        + Send
        + Sync,
>;

/// Per-route metadata, collected at registration time so we can emit an
/// OpenAPI spec without re-walking the trie.
#[derive(Debug, Clone, Default)]
pub struct RouteMeta {
    pub path: String,
    pub method: Method,
    /// Optional JSON Schema describing the request body.
    pub request_schema: Option<serde_json::Value>,
    /// Optional JSON Schema describing the response body.
    pub response_schema: Option<serde_json::Value>,
    /// Human-readable summary, used in `/docs`.
    pub summary: Option<String>,
    /// Tags for grouping in OpenAPI.
    pub tags: Vec<String>,
}

struct Node {
    /// Static children, keyed by exact segment literal.
    static_children: HashMap<String, Node>,
    /// Parameter child (`:name`). Only one per node — first registered wins.
    param_child: Option<(String, Box<Node>)>,
    /// Wildcard child (`*rest`).
    wildcard_child: Option<(String, Box<Node>)>,
    /// Handlers indexed by HTTP method, present at "leaf-ish" nodes.
    handlers: HashMap<Method, Handler>,
    /// Metadata for each registered method (for OpenAPI generation).
    metas: HashMap<Method, RouteMeta>,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            static_children: HashMap::new(),
            param_child: None,
            wildcard_child: None,
            handlers: HashMap::new(),
            metas: HashMap::new(),
        }
    }
}

impl Node {
    fn new() -> Self {
        Self::default()
    }

    fn insert(
        &mut self,
        segments: &[&str],
        method: Method,
        handler: Handler,
        meta: RouteMeta,
    ) -> Result<()> {
        if segments.is_empty() {
            if self.handlers.insert(method, handler).is_some() {
                return Err(KungfuError::new(
                    StatusCode::InternalServerError,
                    format!("Duplicate route: {} {}", method.as_str(), meta.path),
                ));
            }
            self.metas.insert(method, meta);
            return Ok(());
        }

        let seg = segments[0];
        let rest = &segments[1..];

        if let Some(name) = seg.strip_prefix(':') {
            // Ensure a parameter child exists (one per node — first wins).
            if self.param_child.is_none() {
                self.param_child = Some((name.to_string(), Box::new(Node::new())));
            }
            let (_, child) = self.param_child.as_mut().unwrap();
            child.insert(rest, method, handler, meta)?;
        } else if let Some(name) = seg.strip_prefix('*') {
            if self.wildcard_child.is_none() {
                self.wildcard_child = Some((name.to_string(), Box::new(Node::new())));
            }
            let (_, child) = self.wildcard_child.as_mut().unwrap();
            child.insert(rest, method, handler, meta)?;
        } else {
            let child = self.static_children.entry(seg.to_string()).or_insert_with(Node::new);
            child.insert(rest, method, handler, meta)?;
        }
        Ok(())
    }

    fn lookup<'a>(
        &'a self,
        segments: &[&str],
        params: &mut HashMap<String, String>,
    ) -> Option<&'a Node> {
        if segments.is_empty() {
            return Some(self);
        }

        let seg = segments[0];
        let rest = &segments[1..];

        if let Some(child) = self.static_children.get(seg) {
            if let Some(found) = child.lookup(rest, params) {
                return Some(found);
            }
        }

        if let Some((name, child)) = &self.param_child {
            params.insert(name.clone(), seg.to_string());
            if let Some(found) = child.lookup(rest, params) {
                return Some(found);
            }
            params.remove(name);
        }

        if let Some((name, child)) = &self.wildcard_child {
            // Wildcard captures the current segment + all remaining segments.
            params.insert(name.clone(), segments.join("/"));
            return Some(child.as_ref());
        }

        None
    }
}

/// The top-level router. Owns the trie plus the registered middleware chain.
pub struct Router {
    root: Node,
    /// Middleware applied to every request that hits this router, in order.
    middleware: Vec<crate::middleware::Middleware>,
    /// Static fallback handler for 404s (e.g. serve files from public/).
    fallback: Option<Handler>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            root: Node::new(),
            middleware: Vec::new(),
            fallback: None,
        }
    }

    fn split_path(path: &str) -> Vec<&str> {
        path.split('/').filter(|s| !s.is_empty()).collect()
    }

    pub fn add(&mut self, method: Method, path: &str, handler: Handler, meta: RouteMeta) -> Result<()> {
        let segments = Self::split_path(path);
        self.root.insert(&segments, method, handler, meta)
    }

    pub fn get(&mut self, path: &str, handler: Handler) -> Result<()> {
        self.add(Method::Get, path, handler, RouteMeta {
            path: path.into(),
            method: Method::Get,
            ..Default::default()
        })
    }

    pub fn post(&mut self, path: &str, handler: Handler) -> Result<()> {
        self.add(Method::Post, path, handler, RouteMeta {
            path: path.into(),
            method: Method::Post,
            ..Default::default()
        })
    }

    pub fn put(&mut self, path: &str, handler: Handler) -> Result<()> {
        self.add(Method::Put, path, handler, RouteMeta {
            path: path.into(),
            method: Method::Put,
            ..Default::default()
        })
    }

    pub fn delete(&mut self, path: &str, handler: Handler) -> Result<()> {
        self.add(Method::Delete, path, handler, RouteMeta {
            path: path.into(),
            method: Method::Delete,
            ..Default::default()
        })
    }

    pub fn patch(&mut self, path: &str, handler: Handler) -> Result<()> {
        self.add(Method::Patch, path, handler, RouteMeta {
            path: path.into(),
            method: Method::Patch,
            ..Default::default()
        })
    }

    /// Register a route with full metadata — used by the OpenAPI generator.
    pub fn add_with_meta(&mut self, meta: RouteMeta, handler: Handler) -> Result<()> {
        let method = meta.method;
        let path = meta.path.clone();
        self.add(method, &path, handler, meta)
    }

    pub fn use_middleware(&mut self, mw: crate::middleware::Middleware) {
        self.middleware.push(mw);
    }

    /// Prepend a middleware at the front of the chain (used internally to
    /// build the default secure-by-default stack).
    pub fn prepend_middleware(&mut self, mw: crate::middleware::Middleware) {
        self.middleware.insert(0, mw);
    }

    pub fn set_fallback(&mut self, handler: Handler) {
        self.fallback = Some(handler);
    }

    /// Resolve a (method, path) tuple to a handler + extracted params.
    pub fn resolve(&self, method: Method, path: &str) -> RouteResolution {
        let segments = Self::split_path(path);
        let mut params = HashMap::new();
        match self.root.lookup(&segments, &mut params) {
            Some(node) => {
                if let Some(handler) = node.handlers.get(&method) {
                    RouteResolution::Found {
                        handler: handler.clone(),
                        params,
                        meta: node.metas.get(&method).cloned(),
                    }
                } else if !node.handlers.is_empty() {
                    RouteResolution::MethodNotAllowed
                } else {
                    RouteResolution::NotFound
                }
            }
            None => RouteResolution::NotFound,
        }
    }

    pub fn fallback(&self) -> Option<&Handler> {
        self.fallback.as_ref()
    }

    pub fn middleware(&self) -> &[crate::middleware::Middleware] {
        &self.middleware
    }

    /// Walk the trie and produce a flat list of all registered routes —
    /// used by the OpenAPI generator.
    pub fn routes(&self) -> Vec<RouteMeta> {
        let mut out = Vec::new();
        Self::collect_routes(&self.root, &mut out);
        out
    }

    fn collect_routes(node: &Node, out: &mut Vec<RouteMeta>) {
        for meta in node.metas.values() {
            out.push(meta.clone());
        }
        for child in node.static_children.values() {
            Self::collect_routes(child, out);
        }
        if let Some((_, child)) = &node.param_child {
            Self::collect_routes(child, out);
        }
        if let Some((_, child)) = &node.wildcard_child {
            Self::collect_routes(child, out);
        }
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

pub enum RouteResolution {
    Found {
        handler: Handler,
        params: HashMap<String, String>,
        meta: Option<RouteMeta>,
    },
    NotFound,
    MethodNotAllowed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::response::Response;

    fn handler_singleton(body: &'static str) -> Handler {
        std::sync::Arc::new(move |_req: Request| {
            Box::pin(async move { Response::new().text(body) })
        })
    }

    #[test]
    fn routes_static_path() {
        let mut r = Router::new();
        r.get("/hello", handler_singleton("hi")).unwrap();
        match r.resolve(Method::Get, "/hello") {
            RouteResolution::Found { .. } => {}
            _ => panic!("expected Found"),
        }
    }

    #[test]
    fn routes_with_param() {
        let mut r = Router::new();
        r.get("/users/:id", handler_singleton("user")).unwrap();
        match r.resolve(Method::Get, "/users/42") {
            RouteResolution::Found { params, .. } => {
                assert_eq!(params.get("id"), Some(&"42".to_string()));
            }
            _ => panic!("expected Found"),
        }
    }

    #[test]
    fn routes_with_wildcard() {
        let mut r = Router::new();
        r.get("/assets/*path", handler_singleton("asset")).unwrap();
        match r.resolve(Method::Get, "/assets/css/app.css") {
            RouteResolution::Found { params, .. } => {
                assert_eq!(params.get("path"), Some(&"css/app.css".to_string()));
            }
            _ => panic!("expected Found"),
        }
    }

    #[test]
    fn returns_method_not_allowed_for_known_path_wrong_method() {
        let mut r = Router::new();
        r.get("/users", handler_singleton("list")).unwrap();
        assert!(matches!(
            r.resolve(Method::Post, "/users"),
            RouteResolution::MethodNotAllowed
        ));
    }

    #[test]
    fn returns_not_found_for_unknown_path() {
        let mut r = Router::new();
        r.get("/users", handler_singleton("list")).unwrap();
        assert!(matches!(
            r.resolve(Method::Get, "/nope"),
            RouteResolution::NotFound
        ));
    }

    #[test]
    fn collects_all_registered_routes() {
        let mut r = Router::new();
        r.get("/a", handler_singleton("a")).unwrap();
        r.post("/b/:id", handler_singleton("b")).unwrap();
        let routes = r.routes();
        assert_eq!(routes.len(), 2);
    }
}

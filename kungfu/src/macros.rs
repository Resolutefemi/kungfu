//! Proc macros for route registration.
//!
//! We can't use a real proc-macro crate here (it would have to be a separate
//! crate in the workspace), so for V1 we use a declarative `macro_rules!`
//! approach. Each `#[get("/path")]` expands to a function that returns a
//! `(RouteMeta, Handler)` pair, which `Kungfu::route` then registers.
//!
//! For Phase 2 we'll move these into a `kungfu-macros` proc-macro crate so
//! they look exactly like the spec (no `register!` wrapper needed).

/// Registers a handler for `GET /path`.
#[macro_export]
macro_rules! get {
    ($path:expr, $handler:expr) => {{
        use $crate::__macro_support::*;
        const __PATH: &str = $path;
        fn __register(router: &mut Router) -> $crate::Result<()> {
            let handler: $crate::Handler = make_handler($handler);
            router.add_with_meta(
                $crate::RouteMeta {
                    path: __PATH.to_string(),
                    method: $crate::Method::Get,
                    ..Default::default()
                },
                handler,
            )
        }
        __register
    }};
    ($path:expr) => {{
        use $crate::__macro_support::*;
        const __PATH: &str = $path;
        fn __register(router: &mut Router) -> $crate::Result<()> {
            let handler: $crate::Handler = make_handler(__handler_fn);
            router.add_with_meta(
                $crate::RouteMeta {
                    path: __PATH.to_string(),
                    method: $crate::Method::Get,
                    ..Default::default()
                },
                handler,
            )
        }
        __register
    }};
}

#[macro_export]
macro_rules! post {
    ($path:expr, $handler:expr) => {{
        use $crate::__macro_support::*;
        const __PATH: &str = $path;
        fn __register(router: &mut Router) -> $crate::Result<()> {
            let handler: $crate::Handler = make_handler($handler);
            router.add_with_meta(
                $crate::RouteMeta {
                    path: __PATH.to_string(),
                    method: $crate::Method::Post,
                    ..Default::default()
                },
                handler,
            )
        }
        __register
    }};
}

#[macro_export]
macro_rules! put {
    ($path:expr, $handler:expr) => {{
        use $crate::__macro_support::*;
        const __PATH: &str = $path;
        fn __register(router: &mut Router) -> $crate::Result<()> {
            let handler: $crate::Handler = make_handler($handler);
            router.add_with_meta(
                $crate::RouteMeta {
                    path: __PATH.to_string(),
                    method: $crate::Method::Put,
                    ..Default::default()
                },
                handler,
            )
        }
        __register
    }};
}

#[macro_export]
macro_rules! delete {
    ($path:expr, $handler:expr) => {{
        use $crate::__macro_support::*;
        const __PATH: &str = $path;
        fn __register(router: &mut Router) -> $crate::Result<()> {
            let handler: $crate::Handler = make_handler($handler);
            router.add_with_meta(
                $crate::RouteMeta {
                    path: __PATH.to_string(),
                    method: $crate::Method::Delete,
                    ..Default::default()
                },
                handler,
            )
        }
        __register
    }};
}

#[macro_export]
macro_rules! patch {
    ($path:expr, $handler:expr) => {{
        use $crate::__macro_support::*;
        const __PATH: &str = $path;
        fn __register(router: &mut Router) -> $crate::Result<()> {
            let handler: $crate::Handler = make_handler($handler);
            router.add_with_meta(
                $crate::RouteMeta {
                    path: __PATH.to_string(),
                    method: $crate::Method::Patch,
                    ..Default::default()
                },
                handler,
            )
        }
        __register
    }};
}

/// Internal helpers used by the route macros. Not part of the public API.
pub mod __macro_support {
    use crate::{Handler, Request, Response};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;

    /// Wrap a synchronous `Fn(Request) -> Response` into a boxed async handler.
    pub fn make_handler<F>(f: F) -> Handler
    where
        F: Fn(Request) -> Response + Send + Sync + 'static,
    {
        let f = Arc::new(f);
        Arc::new(move |req: Request| {
            let f = f.clone();
            Box::pin(async move { f(req) }) as Pin<Box<dyn Future<Output = Response> + Send>>
        })
    }

    /// Wrap an `async Fn(Request) -> Response` into a boxed async handler.
    pub fn make_async_handler<F, Fut>(f: F) -> Handler
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Response> + Send + 'static,
    {
        Arc::new(move |req: Request| Box::pin(f(req)))
    }
}

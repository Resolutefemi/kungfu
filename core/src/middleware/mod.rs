//! Middleware composition.
//!
//! Middlewares are async functions that wrap a request. They can short-circuit
//! (return a Response without calling next) or transform the Response after.
//! We use the classic "onion" model: outermost middleware runs first on the
//! way in and last on the way out.

use crate::request::Request;
use crate::response::Response;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub mod builtin;

/// A boxed future returned by `next` inside a middleware.
pub type NextFuture = Pin<Box<dyn Future<Output = Response> + Send>>;

/// `next` is called to invoke the next middleware (or the route handler).
/// Returning a `Response` directly short-circuits the chain.
pub type Next = Arc<dyn Fn(Request) -> NextFuture + Send + Sync>;

/// Middleware signature.
pub type Middleware = Arc<
    dyn Fn(Request, Next) -> Pin<Box<dyn Future<Output = Response> + Send>>
        + Send
        + Sync,
>;

/// Build a `Next` that runs the rest of the chain starting from `index`.
pub fn build_chain(
    middleware: &[Middleware],
    handler: crate::router::Handler,
) -> Next {
    build_chain_at(middleware, 0, handler)
}

fn build_chain_at(
    middleware: &[Middleware],
    index: usize,
    handler: crate::router::Handler,
) -> Next {
    if index >= middleware.len() {
        // End of chain — invoke the handler.
        Arc::new(move |req: Request| {
            let h = handler.clone();
            Box::pin(async move { h(req).await })
        })
    } else {
        let mw = middleware[index].clone();
        let next = build_chain_at(middleware, index + 1, handler);
        Arc::new(move |req: Request| {
            let mw = mw.clone();
            let next = next.clone();
            Box::pin(async move { mw(req, next).await })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::{Method, Request};
    use crate::response::Response;

    fn handler_singleton() -> crate::router::Handler {
        Arc::new(|_req: Request| Box::pin(async { Response::new().text("handler") }))
    }

    fn mw_add_header(value: &'static str) -> Middleware {
        Arc::new(move |req: Request, next: Next| {
            let value = value.to_string();
            Box::pin(async move {
                let mut resp = next(req).await;
                resp.set_header("x-mw", value.clone());
                resp
            })
        })
    }

    #[tokio::test]
    async fn chain_runs_in_order_with_outermost_first() {
        let mws = vec![
            mw_add_header("outer"),
            // Both middlewares set x-mw on the response. In the onion model,
            // outer runs first on the way in and LAST on the way out, so it
            // gets the final word on the response — final value should be "outer".
            mw_add_header("inner"),
        ];
        let next = build_chain(&mws, handler_singleton());
        let req = Request::new(Method::Get, "/");
        let resp = next(req).await;
        assert_eq!(resp.header_value("x-mw"), Some("outer"));
        assert_eq!(resp.body, b"handler"[..]);
    }

    #[tokio::test]
    async fn middleware_can_short_circuit() {
        let mw: Middleware = Arc::new(|_req: Request, _next: Next| {
            Box::pin(async { Response::new().text("blocked") })
        });
        let next = build_chain(&[mw], handler_singleton());
        let req = Request::new(Method::Get, "/");
        let resp = next(req).await;
        assert_eq!(resp.body, b"blocked"[..]);
    }
}

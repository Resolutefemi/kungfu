//! Request logger.

use std::time::Instant;

use crate::middleware::{Middleware, Next};
use crate::request::Request;
use std::sync::Arc;

pub fn logger() -> Middleware {
    Arc::new(|req: Request, next: Next| {
        Box::pin(async move {
            let start = Instant::now();
            let method = req.method.as_str().to_string();
            let path = req.path.clone();
            let remote = req
                .remote_addr
                .map(|a| a.to_string())
                .unwrap_or_else(|| "-".to_string());

            let resp = next(req).await;

            let elapsed = start.elapsed();
            tracing::info!(
                method = %method,
                path = %path,
                status = resp.status.as_u16(),
                remote = %remote,
                elapsed_us = elapsed.as_micros(),
                "{} {} → {} ({}μs)",
                method,
                path,
                resp.status.as_u16(),
                elapsed.as_micros(),
            );
            resp
        })
    })
}

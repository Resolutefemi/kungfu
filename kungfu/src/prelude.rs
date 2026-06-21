//! Re-exports the most commonly used items.

pub use crate::builder::{Kungfu, KungfuBuilder};
pub use crate::{delete, get, patch, post, put};
pub use kungfu_core::{
    error::{KungfuError, Result, StatusCode},
    middleware::{Middleware, Next},
    middleware_builtin,
    request::{Method, Request},
    response::Response,
    router::{Handler, RouteMeta, Router},
    server::Server,
};

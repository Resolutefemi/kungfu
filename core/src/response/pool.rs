//! Response object pooling.
//!
//! Every request allocates a fresh `Response` (which contains a `BTreeMap`
//! for headers — a heap allocation). For high-throughput servers, this
//! allocation is a measurable percentage of request handling time.
//!
//! This module provides a `ResponsePool` that recycles `Response` objects.
//! After a response is sent to the wire, the caller can return it to the
//! pool via `release()`. The next request acquires a pre-built `Response`,
//! resets its state (clears headers + body), and reuses it.
//!
//! The pool is gated behind `parking_lot::Mutex` so it's safe to share
//! across worker threads.

use std::sync::Arc;

use parking_lot::Mutex;

use crate::response::Response;

const DEFAULT_POOL_CAPACITY: usize = 256;

/// A pool of reusable `Response` objects.
#[derive(Clone)]
pub struct ResponsePool {
    inner: Arc<Mutex<Vec<Response>>>,
}

impl ResponsePool {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_POOL_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let mut v = Vec::with_capacity(capacity);
        // Pre-warm the pool.
        for _ in 0..capacity {
            v.push(Response::new());
        }
        Self {
            inner: Arc::new(Mutex::new(v)),
        }
    }

    /// Acquire a `Response` from the pool, or create a new one if empty.
    pub fn acquire(&self) -> Response {
        let mut guard = self.inner.lock();
        match guard.pop() {
            Some(mut r) => {
                r.reset();
                r
            }
            None => Response::new(),
        }
    }

    /// Return a `Response` to the pool for reuse.
    pub fn release(&self, resp: Response) {
        let mut guard = self.inner.lock();
        // If the pool is full, the Response is dropped — fine.
        if guard.len() < DEFAULT_POOL_CAPACITY * 2 {
            guard.push(resp);
        }
    }

    /// Number of responses currently in the pool.
    pub fn len(&self) -> usize {
        self.inner.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.lock().is_empty()
    }
}

impl Default for ResponsePool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::StatusCode;

    #[test]
    fn pool_acquires_pre_warmed_responses() {
        let pool = ResponsePool::with_capacity(4);
        let _r1 = pool.acquire();
        let _r2 = pool.acquire();
        let _r3 = pool.acquire();
        let _r4 = pool.acquire();
        // 5th acquire should create a new one (pool is empty).
        let r5 = pool.acquire();
        assert_eq!(pool.len(), 0);
        let _ = r5;
    }

    #[test]
    fn release_returns_to_pool() {
        let pool = ResponsePool::with_capacity(2);
        let r1 = pool.acquire();
        let r2 = pool.acquire();
        assert_eq!(pool.len(), 0);

        pool.release(r1);
        pool.release(r2);
        assert_eq!(pool.len(), 2);

        // Next acquire should reuse.
        let _r3 = pool.acquire();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn acquired_response_is_reset() {
        let pool = ResponsePool::new();
        let mut r1 = pool.acquire();
        r1.set_status(StatusCode::NotFound);
        r1.set_header("x-test", "hello");
        r1 = r1.text("body");
        pool.release(r1);

        let r2 = pool.acquire();
        assert_eq!(r2.status, StatusCode::Ok);
        assert!(r2.header_value("x-test").is_none());
        assert!(r2.body.is_empty());
    }
}

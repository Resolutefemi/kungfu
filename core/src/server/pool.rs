//! Buffer pooling for the request hot path.
//!
//! Allocating a fresh `Vec<u8>` for every read is the single biggest source
//! of GC pressure in a Rust HTTP server. We pool buffers per connection
//! using `crossbeam::queue::ArrayQueue` and `parking_lot::Mutex` to keep
//! contention low.

use std::sync::Arc;

use crossbeam::queue::ArrayQueue;
use parking_lot::Mutex;

/// A pool of reusable byte buffers. Each buffer is `cap` bytes.
///
/// Buffers are returned to the pool on `Drop` of the `PooledBuffer` guard,
/// so callers don't need to remember to release them.
pub struct BufferPool {
    queue: ArrayQueue<Vec<u8>>,
    cap: usize,
    /// Number of times we had to allocate a fresh buffer because the pool
    /// was empty. Exposed for stats / benchmarking.
    allocations: Mutex<usize>,
}

impl BufferPool {
    pub fn new(capacity: usize, buffer_size: usize) -> Arc<Self> {
        let queue = ArrayQueue::new(capacity);
        // Pre-warm the pool with `capacity` buffers.
        for _ in 0..capacity {
            let _ = queue.push(Vec::with_capacity(buffer_size));
        }
        Arc::new(Self {
            queue,
            cap: buffer_size,
            allocations: Mutex::new(0),
        })
    }

    /// Take a buffer from the pool, or allocate a new one if empty.
    pub fn acquire(self: &Arc<Self>) -> PooledBuffer {
        let buf = match self.queue.pop() {
            Some(mut b) => {
                b.clear();
                b
            }
            None => {
                let mut allocs = self.allocations.lock();
                *allocs += 1;
                Vec::with_capacity(self.cap)
            }
        };
        PooledBuffer {
            pool: Arc::clone(self),
            buf: Some(buf),
        }
    }

    pub fn allocations(&self) -> usize {
        *self.allocations.lock()
    }

    fn release(&self, mut buf: Vec<u8>) {
        buf.clear();
        // If the queue is full, the buffer is dropped — fine.
        let _ = self.queue.push(buf);
    }
}

/// RAII guard around a pooled buffer. Returns it to the pool on `Drop`.
pub struct PooledBuffer {
    pool: Arc<BufferPool>,
    buf: Option<Vec<u8>>,
}

impl PooledBuffer {
    /// Get a mutable reference to the underlying buffer.
    pub fn as_mut(&mut self) -> &mut Vec<u8> {
        self.buf.as_mut().expect("pooled buffer already taken")
    }

    /// Get a slice of the underlying buffer.
    pub fn as_slice(&self) -> &[u8] {
        self.buf.as_ref().expect("pooled buffer already taken")
    }

    /// Take ownership of the buffer's contents (clones the bytes).
    /// The pooled buffer itself is still returned to the pool on Drop.
    pub fn to_owned(&self) -> Vec<u8> {
        self.as_slice().to_vec()
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        if let Some(buf) = self.buf.take() {
            self.pool.release(buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_reuses_buffers() {
        let pool = BufferPool::new(4, 4096);
        let initial_allocs = pool.allocations();

        {
            let mut a = pool.acquire();
            a.as_mut().extend_from_slice(b"hello");
            let allocs_after_acquire = pool.allocations();
            assert_eq!(allocs_after_acquire, initial_allocs); // came from the pre-warmed pool
        }

        // After drop, the buffer should be back in the pool — next acquire
        // should not allocate.
        let _b = pool.acquire();
        let _c = pool.acquire();
        let _d = pool.acquire();
        let _e = pool.acquire();
        assert_eq!(pool.allocations(), initial_allocs);

        // 5th acquire exhausts the pool — should allocate.
        let _f = pool.acquire();
        assert_eq!(pool.allocations(), initial_allocs + 1);
    }
}

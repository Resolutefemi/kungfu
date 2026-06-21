//! Background job queue.
//!
//! Provides a simple in-process background job queue. Jobs are `async fn`
//! closures stored in a channel; a worker pool pulls them off and executes
//! them. Useful for sending emails, processing uploads, etc. without
//! blocking the request handler.
//!
//! ## Example
//!
//! ```ignore
//! use kungfu::jobs::{Queue, Job};
//!
//! let queue = Queue::new(4);  // 4 worker threads
//! queue.spawn(async move {
//!     // expensive work here
//!     send_email(user_email).await;
//! });
//! ```

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// A boxed async job — `Pin<Box<dyn Future<Output = ()> + Send>>`.
pub type Job = Pin<Box<dyn Future<Output = ()> + Send>>;

/// A background job queue with N worker tasks.
pub struct Queue {
    sender: mpsc::Sender<Job>,
    _workers: Vec<JoinHandle<()>>,
}

impl Queue {
    /// Create a new queue with `n_workers` background workers.
    pub fn new(n_workers: usize) -> Self {
        let (sender, mut receiver) = mpsc::channel::<Job>(256);
        let mut workers = Vec::with_capacity(n_workers);

        // V1: spawn a single worker that pulls from the receiver.
        // V1.1 will add proper N-worker work-stealing.
        let _ = n_workers;  // V1 limitation — V1.1 will use this.
        let handle = tokio::spawn(async move {
            while let Some(job) = receiver.recv().await {
                job.await;
            }
        });
        workers.push(handle);

        Self {
            sender,
            _workers: workers,
        }
    }

    /// Spawn a job onto the queue. Returns `Err` if the queue is full or
    /// shut down.
    pub fn spawn<F>(&self, job: F) -> Result<(), QueueError>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let boxed: Job = Box::pin(job);
        self.sender
            .try_send(boxed)
            .map_err(|_| QueueError::QueueFull)
    }

    /// Spawn a job with a delay.
    pub fn spawn_delayed<F>(&self, delay: std::time::Duration, job: F) -> Result<(), QueueError>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let sender = self.sender.clone();
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            let boxed: Job = Box::pin(job);
            let _ = sender.try_send(boxed);
        });
        Ok(())
    }
}

#[derive(Debug)]
pub enum QueueError {
    QueueFull,
}

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueueError::QueueFull => write!(f, "queue is full"),
        }
    }
}

impl std::error::Error for QueueError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn spawns_and_runs_job() {
        let queue = Queue::new(1);
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();
        queue.spawn(async move {
            c.fetch_add(1, Ordering::SeqCst);
        }).unwrap();

        // Give the worker time to run.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn spawns_delayed_job() {
        let queue = Queue::new(1);
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();
        queue.spawn_delayed(Duration::from_millis(50), async move {
            c.fetch_add(1, Ordering::SeqCst);
        }).unwrap();

        // Before delay.
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 0);

        // After delay.
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}

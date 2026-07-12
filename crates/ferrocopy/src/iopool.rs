//! IoTaskPool — a dedicated I/O thread pool inspired by Bevy's IoTaskPool.
//!
//! Maintains a fixed set of worker threads that execute blocking I/O tasks.
//! Uses crossbeam_channel for work distribution.

use crossbeam_channel::{bounded, Sender};
use std::thread;

/// A pool of threads dedicated to I/O operations.
pub struct IoTaskPool {
    sender: Sender<Box<dyn FnOnce() + Send + 'static>>,
    _handles: Vec<thread::JoinHandle<()>>,
}

impl IoTaskPool {
    /// Create a new pool with `n` worker threads.
    pub fn new(n: usize) -> Self {
        let (tx, rx) = bounded::<Box<dyn FnOnce() + Send + 'static>>(n * 4);
        let mut handles = Vec::with_capacity(n);

        for _ in 0..n {
            let rx = rx.clone();
            handles.push(thread::spawn(move || {
                while let Ok(task) = rx.recv() {
                    task();
                }
            }));
        }

        Self {
            sender: tx,
            _handles: handles,
        }
    }

    /// Spawn a task on the I/O pool.
    pub fn spawn<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let _ = self.sender.send(Box::new(f));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_pool_executes_tasks() {
        let pool = IoTaskPool::new(2);
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        pool.spawn(move || {
            c.fetch_add(1, Ordering::SeqCst);
        });
        let c = counter.clone();
        pool.spawn(move || {
            c.fetch_add(1, Ordering::SeqCst);
        });
        thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
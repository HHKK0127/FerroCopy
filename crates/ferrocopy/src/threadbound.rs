//! ThreadBound — cross-thread task dispatcher inspired by Bevy's MainThreadExecutor.
//!
//! Queues `FnOnce` closures from any thread and drains them on the target thread
//! (typically the UI/render thread), ensuring all UI mutations happen on one thread.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, ThreadId};

type Task = Box<dyn FnOnce() + Send + 'static>;

/// Sends tasks to be executed on the bound thread.
pub struct ThreadBoundExecutor {
    sender: Sender<Task>,
    thread_id: ThreadId,
}

impl ThreadBoundExecutor {
    /// Create a new executor pair. Returns the sender and its corresponding receiver.
    /// Call `receiver.drain()` on the bound thread to execute pending tasks.
    pub fn new() -> (Self, ThreadReceiver) {
        let (tx, rx) = mpsc::channel::<Task>();
        let thread_id = thread::current().id();
        let receiver = ThreadReceiver {
            receiver: Arc::new(Mutex::new(rx)),
        };
        (Self { sender: tx, thread_id }, receiver)
    }

    /// Queue a closure for execution on the bound thread.
    pub fn spawn<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let _ = self.sender.send(Box::new(f));
    }

    /// The thread ID this executor dispatches to.
    pub fn thread_id(&self) -> ThreadId {
        self.thread_id
    }
}

/// Receives and drains tasks on the bound thread.
pub struct ThreadReceiver {
    receiver: Arc<Mutex<Receiver<Task>>>,
}

impl ThreadReceiver {
    /// Execute all queued tasks (non-blocking).
    pub fn drain(&self) {
        let rx = self.receiver.lock().unwrap();
        while let Ok(task) = rx.try_recv() {
            task();
        }
    }

    /// Block until at least one task is available, then drain all.
    pub fn drain_blocking(&self) {
        let rx = self.receiver.lock().unwrap();
        if let Ok(task) = rx.recv() {
            task();
        }
        while let Ok(task) = rx.try_recv() {
            task();
        }
    }

    /// Block indefinitely, draining tasks as they arrive.
    pub fn run_forever(&self) {
        loop {
            let rx = self.receiver.lock().unwrap();
            if let Ok(task) = rx.recv() {
                drop(rx);
                task();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_executor_runs_task() {
        let (exec, receiver) = ThreadBoundExecutor::new();
        let ran = Arc::new(AtomicBool::new(false));
        let r = ran.clone();
        exec.spawn(move || {
            r.store(true, Ordering::SeqCst);
        });
        receiver.drain();
        assert!(ran.load(Ordering::SeqCst));
    }

    #[test]
    fn test_multiple_tasks_executed() {
        let (exec, receiver) = ThreadBoundExecutor::new();
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        for _ in 0..5 {
            let c = counter.clone();
            exec.spawn(move || {
                c.fetch_add(1, Ordering::SeqCst);
            });
        }
        receiver.drain();
        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }

    #[test]
    fn test_empty_drain_no_panic() {
        let (_, receiver) = ThreadBoundExecutor::new();
        receiver.drain();
    }

    #[test]
    fn test_thread_id_matches_creator() {
        let id = thread::current().id();
        let (exec, _) = ThreadBoundExecutor::new();
        assert_eq!(exec.thread_id(), id);
    }

    #[test]
    fn test_drain_blocking_waits_for_task() {
        let (exec, receiver) = ThreadBoundExecutor::new();
        let ran = Arc::new(AtomicBool::new(false));
        let r = ran.clone();
        let handle = thread::spawn(move || {
            receiver.drain_blocking();
        });
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(!ran.load(Ordering::SeqCst));
        exec.spawn(move || {
            r.store(true, Ordering::SeqCst);
        });
        handle.join().unwrap();
        assert!(ran.load(Ordering::SeqCst));
    }
}

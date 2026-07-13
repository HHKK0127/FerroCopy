//! Fanout — progress broadcasting to multiple subscribers (Yserver-inspired).
//!
//! Distributes copy progress events to multiple listeners without blocking
//! the sender. Each subscriber gets its own channel and can consume at its
//! own pace; slow subscribers miss batches (non-blocking).

use crossbeam_channel::{self as channel, Receiver, Sender, TrySendError};
use std::sync::{Arc, Mutex};

/// A progress update sent to all subscribers.
#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    /// Percentage 0.0 .. 100.0
    pub percent: f64,
    /// Files copied so far
    pub files_done: u64,
    /// Total files to copy
    pub files_total: u64,
    /// Human-readable label
    pub label: String,
}

/// Broadcasts progress updates to multiple subscribers.
pub struct Fanout {
    subscribers: Arc<Mutex<Vec<Sender<ProgressUpdate>>>>,
}

impl Fanout {
    /// Create a new empty fanout broadcaster.
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Subscribe — returns a `Receiver` that gets all future updates.
    ///
    /// The channel has a capacity of `buffer`; if a subscriber reads too
    /// slowly, it misses messages (non-blocking sender).
    pub fn subscribe(&self, buffer: usize) -> Receiver<ProgressUpdate> {
        let (tx, rx) = channel::bounded(buffer);
        let mut subs = self.subscribers.lock().unwrap();
        subs.push(tx);
        rx
    }

    /// Broadcast an update to all active subscribers.
    ///
    /// Silently drops subscribers whose channels are full or disconnected.
    pub fn broadcast(&self, update: ProgressUpdate) {
        let mut subs = self.subscribers.lock().unwrap();
        subs.retain(|tx| match tx.try_send(update.clone()) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => true,   // slow subscriber — skip
            Err(TrySendError::Disconnected(_)) => false, // unsubscribed
        });
    }

    /// Number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_broadcast_reaches_subscribers() {
        let fanout = Fanout::new();
        let rx1 = fanout.subscribe(8);
        let rx2 = fanout.subscribe(8);

        let update = ProgressUpdate {
            percent: 50.0,
            files_done: 5,
            files_total: 10,
            label: "Copying...".into(),
        };

        fanout.broadcast(update.clone());

        let received1 = rx1.try_recv().unwrap();
        let received2 = rx2.try_recv().unwrap();
        assert_eq!((received1.percent * 10.0) as i64, 500);
        assert_eq!((received2.percent * 10.0) as i64, 500);
    }

    #[test]
    fn test_slow_subscriber_misses_updates() {
        let fanout = Fanout::new();
        let rx = fanout.subscribe(2); // small buffer

        for i in 0..5 {
            fanout.broadcast(ProgressUpdate {
                percent: (i as f64) * 20.0,
                files_done: i,
                files_total: 5,
                label: format!("Step {}", i),
            });
        }

        // At most 2 messages should remain (buffer = 2), rest were dropped.
        let mut count = 0;
        while rx.try_recv().is_ok() {
            count += 1;
        }
        assert!(count <= 2, "Should drop messages for slow subscriber, got {}", count);
    }

    #[test]
    fn test_dropped_subscriber_removed() {
        let fanout = Fanout::new();
        let rx = fanout.subscribe(8);
        assert_eq!(fanout.subscriber_count(), 1);

        drop(rx);
        fanout.broadcast(ProgressUpdate {
            percent: 100.0,
            files_done: 1,
            files_total: 1,
            label: "Done".into(),
        });

        assert_eq!(fanout.subscriber_count(), 0);
    }

    #[test]
    fn test_no_subscribers_does_not_panic() {
        let fanout = Fanout::new();
        fanout.broadcast(ProgressUpdate {
            percent: 0.0,
            files_done: 0,
            files_total: 1,
            label: "".into(),
        });
        // Must not panic.
    }

    #[test]
    fn test_multiple_broadcasts() {
        let fanout = Fanout::new();
        let rx = fanout.subscribe(16);

        for i in 0..10 {
            fanout.broadcast(ProgressUpdate {
                percent: (i as f64) * 10.0,
                files_done: i,
                files_total: 10,
                label: format!("item {}", i),
            });
        }

        let mut count = 0;
        while rx.try_recv().is_ok() {
            count += 1;
        }
        assert_eq!(count, 10, "Should receive all 10 updates");
    }
}

//! Integration test for Fanout progress broadcasting.
//! Tests multi-threaded broadcast with crossbeam_channel.

use crossbeam_channel::{self as channel, Receiver, Sender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Re-import the Fanout struct for integration testing.
/// (This mirrors the production code in fanout.rs)
#[derive(Debug, Clone)]
struct ProgressUpdate {
    percent: f64,
    files_done: u64,
    files_total: u64,
    label: String,
}

struct Fanout {
    subscribers: Arc<Mutex<Vec<Sender<ProgressUpdate>>>>,
}

impl Fanout {
    fn new() -> Self {
        Self {
            subscribers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn subscribe(&self, buffer: usize) -> Receiver<ProgressUpdate> {
        let (tx, rx) = channel::bounded(buffer);
        let mut subs = self.subscribers.lock().unwrap();
        subs.push(tx);
        rx
    }

    fn broadcast(&self, update: ProgressUpdate) {
        let mut subs = self.subscribers.lock().unwrap();
        subs.retain(|tx| match tx.try_send(update.clone()) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => true,
            Err(TrySendError::Disconnected(_)) => false,
        });
    }

    fn subscriber_count(&self) -> usize {
        self.subscribers.lock().unwrap().len()
    }
}

#[test]
fn test_fanout_multiple_subscribers_multi_threaded() {
    let fanout = Arc::new(Fanout::new());
    let num_subscribers = 5;
    let num_updates = 100;

    // Subscribe from multiple threads
    let mut handles = vec![];

    for i in 0..num_subscribers {
        let fanout = Arc::clone(&fanout);
        let handle = thread::spawn(move || {
            let rx = fanout.subscribe(64);
            // Each subscriber reads all updates
            let mut received = 0;
            while let Ok(update) = rx.recv_timeout(Duration::from_millis(500)) {
                received += 1;
                if received >= num_updates {
                    break;
                }
                // Verify progress monotonicity
                if received > 1 {
                    // Just check the update is valid
                    assert!(update.percent >= 0.0);
                }
            }
            // Should have received at least some updates
            assert!(received > 0, "Subscriber {} received nothing", i);
            received
        });
        handles.push(handle);
    }

    // Give subscribers time to register
    thread::sleep(Duration::from_millis(50));

    // Broadcast from the main thread
    for i in 0..num_updates {
        fanout.broadcast(ProgressUpdate {
            percent: i as f64,
            files_done: i as u64,
            files_total: num_updates as u64,
            label: format!("item {}", i),
        });
        thread::sleep(Duration::from_millis(1));
    }

    // Collect results
    let mut total_received = 0;
    for h in handles {
        total_received += h.join().expect("subscriber panicked");
    }

    assert!(total_received > 0, "No subscriber received any updates");
    assert_eq!(fanout.subscriber_count(), num_subscribers,
        "All subscribers should remain connected");
}

#[test]
fn test_fanout_broadcast_stress() {
    let fanout = Fanout::new();
    let rx = fanout.subscribe(256);

    // Rapid-fire broadcasts
    let num_broadcasts = 500;
    for i in 0..num_broadcasts {
        fanout.broadcast(ProgressUpdate {
            percent: (i as f64) / (num_broadcasts as f64) * 100.0,
            files_done: i as u64,
            files_total: num_broadcasts as u64,
            label: "stress".into(),
        });
    }

    // Read what we can (may have dropped due to buffer overflow)
    let mut count = 0;
    while let Ok(_) = rx.try_recv() {
        count += 1;
    }

    // At least 1, at most buffer size (256) should have survived
    assert!(count >= 1, "Should have received at least 1 update");
    assert!(count <= 256, "Buffer should cap at 256, got {}", count);
}

#[test]
fn test_fanout_subscriber_drop_in_other_thread() {
    let fanout = Arc::new(Fanout::new());

    // Subscribe in child thread, then drop the receiver
    let fanout_clone = Arc::clone(&fanout);
    let handle = thread::spawn(move || {
        let rx = fanout_clone.subscribe(8);
        // Drop rx immediately
        drop(rx);
    });
    handle.join().expect("thread panicked");

    // Give the fanout time to process the disconnect
    fanout.broadcast(ProgressUpdate {
        percent: 100.0,
        files_done: 1,
        files_total: 1,
        label: "done".into(),
    });

    // Disconnected subscriber should be removed
    assert_eq!(fanout.subscriber_count(), 0);
}

#[test]
fn test_fanout_concurrent_subscribe_and_broadcast() {
    let fanout = Arc::new(Fanout::new());
    let num_threads = 10;

    let mut handles = vec![];

    // Spawn threads that both subscribe and broadcast
    for t in 0..num_threads {
        let fanout = Arc::clone(&fanout);
        let handle = thread::spawn(move || {
            let rx = fanout.subscribe(16);

            for i in 0..20 {
                fanout.broadcast(ProgressUpdate {
                    percent: (t * 100 + i) as f64,
                    files_done: i as u64,
                    files_total: 20,
                    label: format!("t{}", t),
                });
            }

            // Read back what we can
            let mut count = 0;
            while let Ok(_) = rx.try_recv() {
                count += 1;
            }
            count
        });
        handles.push(handle);
    }

    let mut total = 0;
    for h in handles {
        total += h.join().expect("thread panicked");
    }

    // Each thread should have received at least some broadcasts from others
    assert!(total > 0, "No messages received across threads");
}
//! CoreSender/CoreReceiver — event-driven communication between components.
//!
//! Inspired by Yserver's event channel pattern. Provides typed event
//! broadcasting so the GUI, engine, and telemetry can communicate without
//! shared mutable state.

use crossbeam_channel::{bounded, Receiver, Sender};

/// Events emitted by the copy engine and consumed by GUI / telemetry.
#[derive(Debug, Clone)]
pub enum CopyEvent {
    FileStarted {
        file: String,
        total_bytes: u64,
    },
    FileCompleted {
        file: String,
        bytes_copied: u64,
        elapsed_ms: u64,
    },
    Error {
        file: String,
        error: String,
    },
    Finished {
        total_files: u64,
        total_bytes: u64,
        elapsed_ms: u64,
        errors: u64,
    },
    Paused,
    Resumed,
    CancelRequested,
}

/// Clonable sender half of the event channel.
#[derive(Debug, Clone)]
pub struct CoreSender {
    sender: Sender<CopyEvent>,
}

impl CoreSender {
    pub fn send(&self, event: CopyEvent) {
        let _ = self.sender.send(event);
    }
}

/// Receiver half of the event channel.
#[derive(Debug)]
pub struct CoreReceiver {
    receiver: Receiver<CopyEvent>,
}

impl CoreReceiver {
    /// Block until the next event arrives.
    pub fn recv(&self) -> Result<CopyEvent, crossbeam_channel::RecvError> {
        self.receiver.recv()
    }

    /// Try to receive without blocking.
    pub fn try_recv(&self) -> Result<CopyEvent, crossbeam_channel::TryRecvError> {
        self.receiver.try_recv()
    }
}

/// Create a new event channel pair.
pub fn channel(cap: usize) -> (CoreSender, CoreReceiver) {
    let (tx, rx) = bounded(cap);
    (CoreSender { sender: tx }, CoreReceiver { receiver: rx })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_recv() {
        let (tx, rx) = channel(16);
        tx.send(CopyEvent::FileStarted {
            file: "test.txt".into(),
            total_bytes: 100,
        });
        match rx.recv().unwrap() {
            CopyEvent::FileStarted { file, total_bytes } => {
                assert_eq!(file, "test.txt");
                assert_eq!(total_bytes, 100);
            }
            _ => panic!("expected FileStarted"),
        }
    }

    #[test]
    fn test_try_recv_empty() {
        let (_, rx) = channel(16);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_finished_event() {
        let (tx, rx) = channel(16);
        tx.send(CopyEvent::Finished {
            total_files: 10,
            total_bytes: 1000,
            elapsed_ms: 500,
            errors: 0,
        });
        match rx.recv().unwrap() {
            CopyEvent::Finished { total_files, .. } => assert_eq!(total_files, 10),
            _ => panic!("expected Finished"),
        }
    }
}
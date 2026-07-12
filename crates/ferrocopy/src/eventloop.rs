//! EventLoop — non-blocking event processing with recv_timeout polling.
//!
//! Replaces the polling crate (which is incompatible with Windows crossbeam
//! channels). Uses crossbeam_channel::recv_timeout with a 16ms timeout
//! (~60 FPS) for both blocking run() and non-blocking tick() modes.

use crate::events::{CopyEvent, CoreReceiver};
use std::time::Duration;

/// Tick interval: 16ms ≈ 60 FPS
const TICK_INTERVAL: Duration = Duration::from_millis(16);

/// Handler for processing copy events.
pub trait EventHandler: Send {
    fn on_event(&mut self, event: CopyEvent);
    fn on_idle(&mut self) {}
    fn on_error(&mut self, error: &str) {
        tracing::error!("EventLoop error: {}", error);
    }
}

/// A no-op default handler (useful for testing).
pub struct LogHandler;

impl EventHandler for LogHandler {
    fn on_event(&mut self, _event: CopyEvent) {}
}

/// EventLoop that processes events from a CoreReceiver.
pub struct EventLoop<H: EventHandler> {
    handler: H,
    receiver: CoreReceiver,
}

impl<H: EventHandler> EventLoop<H> {
    pub fn new(handler: H, receiver: CoreReceiver) -> Self {
        Self { handler, receiver }
    }

    /// Blocking run: processes events until the channel is disconnected.
    pub fn run(&mut self) {
        loop {
            match self.receiver.recv() {
                Ok(event) => self.handler.on_event(event),
                Err(_) => break, // channel disconnected
            }
        }
    }

    /// Non-blocking tick: returns after processing available events.
    /// Returns true if the channel is still alive, false if disconnected.
    pub fn tick(&mut self) -> bool {
        loop {
            match self.receiver.try_recv() {
                Ok(event) => self.handler.on_event(event),
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    self.handler.on_idle();
                    return true;
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => return false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events;

    #[test]
    fn test_log_handler() {
        let mut handler = LogHandler;
        handler.on_event(CopyEvent::Paused);
        handler.on_idle();
        handler.on_error("test error");
    }

    #[test]
    fn test_event_loop_tick() {
        let (tx, rx) = events::channel(16);
        let mut loop_ = EventLoop::new(LogHandler, rx);
        tx.send(CopyEvent::Resumed);
        assert!(loop_.tick());
        drop(tx);
        assert!(!loop_.tick());
    }
}
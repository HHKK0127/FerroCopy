//! Signal — graceful shutdown handling.
//!
//! Inspired by Yserver and Alacritty. Provides a cross-platform
//! shutdown signal (ctrl-c on all platforms).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Register a ctrl-c handler that sets the given AtomicBool to true.
/// Returns Err if a handler was already registered.
pub fn register_shutdown_signal(shutdown: Arc<AtomicBool>) -> Result<(), String> {
    ctrlc::set_handler(move || {
        shutdown.store(true, Ordering::SeqCst);
        tracing::info!("Shutdown signal received");
    })
    .map_err(|e| format!("Failed to register ctrl-c handler: {}", e))
}

/// Wait for the shutdown signal to be raised (polling).
/// Returns immediately if already set.
pub fn wait_for_shutdown(shutdown: &AtomicBool, poll_ms: u64) {
    while !shutdown.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(poll_ms));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_shutdown_flag() {
        let flag = Arc::new(AtomicBool::new(false));
        assert!(!flag.load(Ordering::SeqCst));
        flag.store(true, Ordering::SeqCst);
        assert!(flag.load(Ordering::SeqCst));
    }

    #[test]
    fn test_wait_immediate() {
        let flag = AtomicBool::new(true);
        // Should return immediately since already set
        wait_for_shutdown(&flag, 10);
    }
}
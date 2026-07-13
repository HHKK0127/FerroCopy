//! Telemetry — periodic diagnostic logging.
//!
//! Inspired by Yserver's loop telemetry system. Collects copy metrics
//! at 1-second intervals and logs them via tracing::info!.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Snapshot of copy metrics at a point in time.
#[derive(Debug, Clone, Default)]
pub struct TelemetrySnapshot {
    pub bytes_copied: u64,
    pub files_completed: u64,
    pub errors: u64,
    pub elapsed_secs: f64,
}

/// Shared telemetry counters accessible from the engine and the GUI.
#[derive(Debug, Default)]
pub struct TelemetryCounters {
    pub bytes_copied: Arc<AtomicU64>,
    pub files_completed: Arc<AtomicU64>,
    pub errors: Arc<AtomicU64>,
}

impl TelemetryCounters {
    pub fn new() -> Self {
        Self::default()
    }

    /// Take a snapshot of current values.
    pub fn snapshot(&self) -> TelemetrySnapshot {
        TelemetrySnapshot {
            bytes_copied: self.bytes_copied.load(Ordering::Relaxed),
            files_completed: self.files_completed.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            elapsed_secs: 0.0,
        }
    }
}

/// Runs a diagnostic loop that logs telemetry every `interval`.
pub fn spawn_telemetry_loop(
    counters: Arc<TelemetryCounters>,
    interval: Duration,
) -> std::thread::JoinHandle<()> {
    let start = std::time::Instant::now();
    let mut prev = TelemetrySnapshot::default();

    std::thread::spawn(move || loop {
        std::thread::sleep(interval);

        let now = counters.snapshot();
        let elapsed = start.elapsed().as_secs_f64();
        let dt = elapsed - prev.elapsed_secs;

        if dt > 0.0 {
            let bytes_per_sec = (now.bytes_copied - prev.bytes_copied) as f64 / dt;
            let files_per_sec = (now.files_completed - prev.files_completed) as f64 / dt;
            let total_errors = now.errors;

            tracing::info!(
                "📊 {:.0} s | {:.1} MB/s | {:.2} files/s | {} errors",
                elapsed,
                bytes_per_sec / 1_000_000.0,
                files_per_sec,
                total_errors,
            );
        }

        prev = now;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_snapshot() {
        let counters = TelemetryCounters::new();
        counters.bytes_copied.store(42, Ordering::SeqCst);
        counters.files_completed.store(5, Ordering::SeqCst);
        let snap = counters.snapshot();
        assert_eq!(snap.bytes_copied, 42);
        assert_eq!(snap.files_completed, 5);
    }
}
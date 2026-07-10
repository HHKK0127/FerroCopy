// ── Yserver/Alacritty-inspired: Graceful Shutdown ────────────────────
//
// SIGINT/SIGTERM handling: capture Ctrl+C, save intermediate EngineResult,
// and allow graceful cleanup instead of abrupt termination.

use tokio::sync::watch;

/// Manages graceful shutdown via OS signals.
/// Yserver-inspired: one-shot channel that fires on signal.
pub struct Shutdown {
    /// Receiver gets notified when shutdown is requested.
    pub rx: watch::Receiver<bool>,
    /// Sender fires the shutdown signal.
    tx: watch::Sender<bool>,
}

impl Shutdown {
    /// Create a new shutdown handler and spawn signal listener.
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            // Wait for SIGINT (Ctrl+C) or SIGTERM
            tokio::signal::ctrl_c().await.ok();
            tracing::info!("Shutdown signal received, cleaning up...");
            let _ = tx_clone.send(true);
        });
        Self { tx, rx }
    }

    /// Check if shutdown has been requested.
    pub fn is_shutdown_requested(&self) -> bool {
        *self.rx.borrow()
    }

    /// Wait for shutdown signal.
    pub async fn wait_for_shutdown(&mut self) {
        while !*self.rx.borrow_and_update() {
            self.rx.changed().await.ok();
        }
    }
}

/// Spawn a task that saves EngineResult on shutdown.
/// Alacritty-inspired: write partial result to temp file for recovery.
pub fn spawn_result_saver(
    mut shutdown: Shutdown,
    result_path: std::path::PathBuf,
    result: std::sync::Arc<std::sync::Mutex<crate::engine::EngineResult>>,
) {
    tokio::spawn(async move {
        shutdown.wait_for_shutdown().await;
        let final_result = result.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let total = final_result.outcomes.len();
        let errors = final_result.count(crate::engine::CopySeverity::Error);
        // Write summary to result file
        if let Ok(content) = serde_json::to_string_pretty(&serde_json::json!({
            "total_files": total,
            "error_count": errors,
            "copied_bytes": final_result.copied_bytes,
            "message": format!("{}", final_result),
        })) {
            let _ = std::fs::write(&result_path, content);
            tracing::info!("Saved shutdown result to {:?}", result_path);
        }
        // Flush and finish
        eprintln!("\n⚠ Shutdown: {} files processed, {} errors\nInterrupted — partial results saved to {:?}", total, errors, result_path);
    });
}
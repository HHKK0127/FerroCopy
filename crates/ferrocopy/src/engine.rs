use crate::config::*;
use crate::hash;
use anyhow::{Context, Result};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Semaphore;
use walkdir::WalkDir;

// ── Warp-inspired: Exponential backoff with jitter ────────────────────

/// Retry strategy with exponential backoff and full jitter.
/// Warp's `sync_queue` inspired: transient errors are retried with
/// exponential backoff + random jitter to avoid thundering herd.
#[derive(Debug, Clone)]
pub struct RetryStrategy {
    /// Maximum number of retry attempts (0 = no retry)
    pub max_attempts: u32,
    /// Initial backoff duration (doubles each attempt)
    pub base_delay: Duration,
    /// Maximum backoff duration cap
    pub max_delay: Duration,
}

impl Default for RetryStrategy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
        }
    }
}

impl RetryStrategy {
    /// Create a new retry strategy with the given parameters.
    pub const fn new(max_attempts: u32, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            max_attempts,
            base_delay,
            max_delay,
        }
    }

    /// Compute the delay for a given attempt (0-based) with full jitter.
    /// delay = min(base * 2^attempt, max_delay) * random(0.0..1.0)
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base_ms = self.base_delay.as_millis() as f64;
        let max_ms = self.max_delay.as_millis() as f64;
        let exponential = (base_ms * (2u64.pow(attempt)) as f64).min(max_ms);
        let jittered = exponential * fastrand::f64(); // full jitter [0.0, 1.0)
        Duration::from_millis(jittered as u64)
    }
}

/// Retry a fallible async operation with exponential backoff + full jitter.
/// Warp's `sync_queue` inspired: transient errors are retried,
/// permanent errors are returned immediately.
///
/// Returns `Ok(result)` on success, or the last error after exhausting retries.
pub async fn retry_with_backoff<T, E, F, Fut>(
    strategy: &RetryStrategy,
    is_transient: fn(&E) -> bool,
    operation: F,
) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let mut last_err = None;
    for attempt in 0..=strategy.max_attempts {
        if attempt > 0 {
            let delay = strategy.delay_for_attempt(attempt - 1);
            tokio::time::sleep(delay).await;
        }
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if !is_transient(&e) || attempt == strategy.max_attempts {
                    return Err(e);
                }
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap())
}

// ── Lore-inspired error severity and aggregation ────────────────────

/// Lore-inspired error severity for result aggregation.
/// Ordered: None < Skipped < Warning < Error < Fatal
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[allow(dead_code)]
pub enum CopySeverity {
    None = 0,
    Skipped = 1,
    Warning = 2,
    Error = 3,
    Fatal = 4,
}

impl fmt::Display for CopySeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CopySeverity::None => write!(f, "OK"),
            CopySeverity::Skipped => write!(f, "SKIP"),
            CopySeverity::Warning => write!(f, "WARN"),
            CopySeverity::Error => write!(f, "ERR"),
            CopySeverity::Fatal => write!(f, "FATAL"),
        }
    }
}

/// Lore-inspired copy outcome: result of copying one file pair.
#[derive(Debug, Clone)]
#[allow(dead_code)]
#[allow(dead_code)]
pub struct CopyOutcome {
    pub src: PathBuf,
    pub dst: PathBuf,
    pub severity: CopySeverity,
    pub message: String,
    pub bytes_copied: u64,
}

impl CopyOutcome {
    pub fn success(src: PathBuf, dst: PathBuf, bytes: u64) -> Self {
        Self {
            src,
            dst,
            severity: CopySeverity::None,
            message: String::new(),
            bytes_copied: bytes,
        }
    }

    #[allow(dead_code)]
    pub fn skipped(src: PathBuf, dst: PathBuf) -> Self {
        Self {
            src,
            dst,
            severity: CopySeverity::Skipped,
            message: "Skipped (already exists)".into(),
            bytes_copied: 0,
        }
    }

    #[allow(dead_code)]
    pub fn warning(src: PathBuf, dst: PathBuf, msg: impl Into<String>) -> Self {
        Self {
            src,
            dst,
            severity: CopySeverity::Warning,
            message: msg.into(),
            bytes_copied: 0,
        }
    }

    pub fn error(src: PathBuf, dst: PathBuf, msg: impl Into<String>) -> Self {
        Self {
            src,
            dst,
            severity: CopySeverity::Error,
            message: msg.into(),
            bytes_copied: 0,
        }
    }

    #[allow(dead_code)]
    pub fn fatal(msg: impl Into<String>) -> Self {
        Self {
            src: PathBuf::new(),
            dst: PathBuf::new(),
            severity: CopySeverity::Fatal,
            message: msg.into(),
            bytes_copied: 0,
        }
    }
}

/// Aggregated result of a batch copy operation.
#[derive(Debug, Clone, Default)]
pub struct EngineResult {
    pub outcomes: Vec<CopyOutcome>,
    pub total_bytes: u64,
    pub copied_bytes: u64,
}

impl EngineResult {
    pub fn new() -> Self {
        Self::default()
    }

    /// Count outcomes at or above the given severity.
    pub fn count(&self, min_severity: CopySeverity) -> usize {
        self.outcomes
            .iter()
            .filter(|o| o.severity >= min_severity)
            .count()
    }

    pub fn errors(&self) -> impl Iterator<Item = &CopyOutcome> {
        self.outcomes.iter().filter(|o| o.severity >= CopySeverity::Error)
    }

    #[allow(dead_code)]
    pub fn has_errors(&self) -> bool {
        self.count(CopySeverity::Error) > 0
    }

    /// Merge another EngineResult into this one (for combining parallel batches).
    #[allow(dead_code)]
    pub fn merge(&mut self, other: EngineResult) {
        self.outcomes.extend(other.outcomes);
        self.total_bytes += other.total_bytes;
        self.copied_bytes += other.copied_bytes;
    }

    fn display_severity_summary(&self) -> String {
        let total = self.outcomes.len();
        let ok = self.count(CopySeverity::None);
        let skipped = self.count(CopySeverity::Skipped) - self.count(CopySeverity::Error); // Skipped excludes Error+
        let errors = self.count(CopySeverity::Error);
        if errors > 0 {
            format!(
                "{} files: {} OK, {} skipped, {} errors",
                total, ok, skipped, errors
            )
        } else if skipped > 0 {
            format!("{} files: {} OK, {} skipped", total, ok, skipped)
        } else {
            format!("{} files: {} OK", total, ok)
        }
    }
}

impl fmt::Display for EngineResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_severity_summary())
    }
}

/// Get file size (for pre-scanning file list in shell integration)
pub fn get_file_size(path: &Path) -> u64 {
    if path.is_file() {
        std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
    } else if path.is_dir() {
        WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| std::fs::metadata(e.path()).ok())
            .map(|m| m.len())
            .sum()
    } else {
        0
    }
}

/// Collect files to copy (recursively or single file)
pub fn collect_files(src: &Path, dest: &Path, recursive: bool) -> Result<Vec<(PathBuf, PathBuf)>> {
    let mut pairs = Vec::new();

    if !src.exists() {
        anyhow::bail!("Source does not exist: {}", src.display());
    }

    if src.is_file() {
        let dest_path = if dest.is_dir()
            || dest
                .to_string_lossy()
                .ends_with(std::path::MAIN_SEPARATOR_STR)
        {
            dest.join(src.file_name().unwrap())
        } else {
            dest.to_path_buf()
        };
        pairs.push((src.to_path_buf(), dest_path));
    } else if src.is_dir() {
        if !recursive {
            anyhow::bail!("Source is a directory. Use --recursive to copy directories.");
        }
        // For directory sources, dest is always treated as a directory.
        // If dest already exists and is a directory, copy INTO it (preserving src's folder name).
        // If dest doesn't exist, treat it as the new directory to create.
        let effective_dest: PathBuf = if dest.is_dir() {
            dest.join(src.file_name().unwrap())
        } else {
            dest.to_path_buf()
        };
        for entry in WalkDir::new(src).min_depth(1) {
            let entry = entry?;
            if entry.file_type().is_file() {
                let rel = entry.path().strip_prefix(src)?;
                let dest_path = effective_dest.join(rel);
                pairs.push((entry.path().to_path_buf(), dest_path));
            }
        }
    }

    Ok(pairs)
}

/// Copy a single file with progress tracking
pub async fn copy_file(
    src: &Path,
    dst: &Path,
    config: &CopyConfig,
    progress_sender: tokio::sync::mpsc::UnboundedSender<FileProgress>,
    cancel: Arc<AtomicBool>,
    pause: Option<Arc<AtomicBool>>,
    bytes_total: Arc<AtomicU64>,
    bytes_done: Arc<AtomicU64>,
) -> Result<()> {
    // Create parent directories
    if let Some(parent) = dst.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Check overwrite
    if dst.exists() {
        match config.overwrite {
            OverwriteMode::Skip => {
                tracing::info!("Skipping existing: {}", dst.display());
                return Ok(());
            }
            OverwriteMode::IfDifferent => {
                // We'll verify after copy if needed; for now, proceed
            }
            OverwriteMode::Always => {}
        }
    }

    let metadata = tokio::fs::metadata(src).await?;
    let file_size = metadata.len();
    bytes_total.fetch_add(file_size, Ordering::SeqCst);

    let mut src_file = tokio::fs::File::open(src)
        .await
        .with_context(|| format!("Failed to open source: {}", src.display()))?;

    let mut dst_file = tokio::fs::File::create(dst)
        .await
        .with_context(|| format!("Failed to create destination: {}", dst.display()))?;

    // Adaptive buffer: small files use smaller buffer, large files use 1MB
    let buf_size = if file_size < 1024 * 1024 {
        64 * 1024 // 64KB
    } else {
        1024 * 1024 // 1MB (TeraCopy-inspired)
    };
    let mut buf = vec![0u8; buf_size];

    let mut copied: u64 = 0;
    let start = std::time::Instant::now();

    // Streaming hash — compute during copy, skip re-read later
    let mut stream_hasher = if config.verify {
        Some(hash::StreamHasher::new(
            matches!(config.hash_algorithm, HashAlgorithm::Blake3),
        ))
    } else {
        None
    };

    loop {
        if cancel.load(Ordering::SeqCst) {
            anyhow::bail!("Copy cancelled by user");
        }

        // Pause support: spin-wait while paused
        if let Some(ref pause_flag) = pause {
            while pause_flag.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(100)).await;
                if cancel.load(Ordering::SeqCst) {
                    anyhow::bail!("Copy cancelled by user");
                }
            }
        }

        let n = src_file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        dst_file.write_all(&buf[..n]).await?;
        copied += n as u64;
        bytes_done.fetch_add(n as u64, Ordering::SeqCst);

        // Streaming hash: update during copy, no re-read needed
        if let Some(ref mut h) = stream_hasher {
            h.update(&buf[..n]);
        }

        let elapsed = start.elapsed().as_secs_f64();
        let speed = if elapsed > 0.0 {
            copied as f64 / elapsed
        } else {
            0.0
        };

        let _ = progress_sender.send(FileProgress {
            file: src.to_path_buf(),
            bytes_copied: copied,
            total_bytes: file_size,
            speed_bytes_per_sec: speed,
        });
    }

    // Preserve timestamps
    if let Ok(meta) = std::fs::metadata(src) {
        if let (Ok(atime), Ok(mtime)) = (meta.accessed(), meta.modified()) {
            let _ = filetime::set_file_times(dst, atime.into(), mtime.into());
        }
    }

    // Verification using streaming hash (no file re-read)
    if config.verify {
        if let Some(h) = stream_hasher {
            let src_hash = h.finalize_hex();
            // Re-hash only the destination file for comparison
            let dst_hash = if matches!(config.hash_algorithm, HashAlgorithm::Blake3) {
                hash::blake3_hash(dst).await?
            } else {
                hash::xxh3_hash(dst).await?
            };
            if src_hash != dst_hash {
                anyhow::bail!("Hash mismatch for: {}", src.display());
            }
            tracing::info!("✓ Hash verified (streaming): {}", src_hash);
        } else {
            // Fallback (shouldn't happen)
            let ok = hash::verify_copy(
                src,
                dst,
                matches!(config.hash_algorithm, HashAlgorithm::Blake3),
            )
            .await?;
            if !ok {
                anyhow::bail!("Hash mismatch for: {}", src.display());
            }
        }
    }

    tracing::info!(
        "Copied: {} → {} ({} bytes)",
        src.display(),
        dst.display(),
        copied
    );
    Ok(())
}

/// Copy a file and optionally delete the source after success (move mode)
/// Warp-inspired: uses exponential backoff + jitter for retries
pub async fn copy_file_with_move(
    src: &Path,
    dst: &Path,
    config: &CopyConfig,
    progress_sender: tokio::sync::mpsc::UnboundedSender<FileProgress>,
    cancel: Arc<AtomicBool>,
    pause: Option<Arc<AtomicBool>>,
    bytes_total: Arc<AtomicU64>,
    bytes_done: Arc<AtomicU64>,
) -> Result<()> {
    copy_file(
        src,
        dst,
        config,
        progress_sender,
        cancel,
        pause,
        bytes_total,
        bytes_done,
    )
    .await?;

    let strategy = RetryStrategy::new(3, Duration::from_millis(100), Duration::from_secs(2));
    retry_with_backoff(
        &strategy,
        |_: &std::io::Error| true, // file deletion errors are always transient
        || async move { tokio::fs::remove_file(src).await },
    )
    .await
    .with_context(|| format!("Failed to delete source after copy: {}", src.display()))?;

    tracing::info!("Moved (deleted source): {}", src.display());
    Ok(())
}

/// Run the copy engine with multiple parallel workers.
/// Returns aggregated results with per-file outcomes.
pub async fn run_copy_engine(
    files: Vec<(PathBuf, PathBuf)>,
    config: Arc<CopyConfig>,
    progress_sender: tokio::sync::mpsc::UnboundedSender<FileProgress>,
    cancel: Arc<AtomicBool>,
    pause: Option<Arc<AtomicBool>>,
    bytes_total: Arc<AtomicU64>,
    bytes_done: Arc<AtomicU64>,
) -> EngineResult {
    run_copy_engine_inner(
        files,
        config,
        progress_sender,
        cancel,
        pause,
        bytes_total,
        bytes_done,
        false,
    )
    .await
}

/// Run copy engine in move mode (deletes source after copy).
/// Returns aggregated results with per-file outcomes.
pub async fn run_move_engine(
    files: Vec<(PathBuf, PathBuf)>,
    config: Arc<CopyConfig>,
    progress_sender: tokio::sync::mpsc::UnboundedSender<FileProgress>,
    cancel: Arc<AtomicBool>,
    pause: Option<Arc<AtomicBool>>,
    bytes_total: Arc<AtomicU64>,
    bytes_done: Arc<AtomicU64>,
) -> EngineResult {
    run_copy_engine_inner(
        files,
        config,
        progress_sender,
        cancel,
        pause,
        bytes_total,
        bytes_done,
        true,
    )
    .await
}

/// ★ Lore-inspired: internal parallel engine with outcome aggregation.
/// Continues on errors (does not stop at first failure).
/// Returns aggregated EngineResult with per-file outcomes.
async fn run_copy_engine_inner(
    files: Vec<(PathBuf, PathBuf)>,
    config: Arc<CopyConfig>,
    progress_sender: tokio::sync::mpsc::UnboundedSender<FileProgress>,
    cancel: Arc<AtomicBool>,
    pause: Option<Arc<AtomicBool>>,
    bytes_total: Arc<AtomicU64>,
    bytes_done: Arc<AtomicU64>,
    move_mode: bool,
) -> EngineResult {
    let semaphore = Arc::new(Semaphore::new(config.threads));
    let result_agg = Arc::new(std::sync::Mutex::new(EngineResult::new()));
    let mut set = tokio::task::JoinSet::new();

    for (src, dst) in files {
        let config = config.clone();
        let sender = progress_sender.clone();
        let cancel = cancel.clone();
        let pause = pause.clone();
        let bt = bytes_total.clone();
        let bd = bytes_done.clone();
        let agg = result_agg.clone();
        let sem = semaphore.clone();

        set.spawn(async move {
            // Acquire semaphore inside spawn — no guard-page gap
            let _permit = sem.acquire_owned().await.unwrap_or_else(|_| {
                panic!("Semaphore closed unexpectedly");
            });

            let outcome = if move_mode {
                match copy_file_with_move(&src, &dst, &config, sender, cancel, pause, bt, bd).await
                {
                    Ok(()) => CopyOutcome::success(src.clone(), dst.clone(), 0),
                    Err(e) => CopyOutcome::error(src.clone(), dst.clone(), format!("{:#}", e)),
                }
            } else {
                match copy_file(&src, &dst, &config, sender, cancel, pause, bt, bd).await {
                    Ok(()) => CopyOutcome::success(src.clone(), dst.clone(), 0),
                    Err(e) => CopyOutcome::error(src.clone(), dst.clone(), format!("{:#}", e)),
                }
            };
            if let Ok(mut agg) = agg.lock() {
                agg.outcomes.push(outcome);
            }
        });
    }

    while let Some(result) = set.join_next().await {
        if let Err(e) = result {
            // Task panicked or cancelled — record as error
            let msg = if e.is_cancelled() {
                "Task cancelled".to_string()
            } else {
                format!("Task panicked: {}", e)
            };
            if let Ok(mut agg) = result_agg.lock() {
                agg.outcomes.push(CopyOutcome::error(
                    PathBuf::from("<unknown>"),
                    PathBuf::from("<unknown>"),
                    msg,
                ));
            }
        }
    }

    // Return aggregated result
    let final_result = result_agg.lock().unwrap_or_else(|e| e.into_inner()).clone();
    final_result
}

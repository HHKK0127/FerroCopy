use crate::config::*;
use crate::hash;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Semaphore;
use walkdir::WalkDir;

/// Get file size (for pre-scanning file list)
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
        let dest_path = if dest.is_dir() || dest.to_string_lossy().ends_with(std::path::MAIN_SEPARATOR_STR) {
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
            OverwriteMode::Always | OverwriteMode::Prompt => {}
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

        let elapsed = start.elapsed().as_secs_f64();
        let speed = if elapsed > 0.0 { copied as f64 / elapsed } else { 0.0 };

        let _ = progress_sender.send(FileProgress {
            file: src.to_path_buf(),
            bytes_copied: copied,
            total_bytes: file_size,
            speed_bytes_per_sec: speed,
        });
    }

    // Preserve timestamps
    if let Ok(meta) = std::fs::metadata(src) {
        if let (Ok(atime), Ok(mtime)) = (
            meta.accessed(),
            meta.modified(),
        ) {
            let _ = filetime::set_file_times(dst, atime.into(), mtime.into());
        }
    }

    // Verification
    if config.verify {
        let ok = hash::verify_copy(src, dst, matches!(config.hash_algorithm, HashAlgorithm::Blake3)).await?;
        if !ok {
            anyhow::bail!("Hash mismatch for: {}", src.display());
        }
    }

    tracing::info!("Copied: {} → {} ({} bytes)", src.display(), dst.display(), copied);
    Ok(())
}

    /// Copy a file and optionally delete the source after success (move mode)
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
        copy_file(src, dst, config, progress_sender, cancel, pause, bytes_total, bytes_done).await?;

        // ★ Lore-inspired: 3-try delete with backoff (simplified JoinSet pattern)
        let mut last_err = None;
        for attempt in 1..=3 {
            match tokio::fs::remove_file(src).await {
                Ok(_) => {
                    tracing::info!("Moved (deleted source): {}", src.display());
                    return Ok(());
                }
                Err(e) => {
                    last_err = Some(e);
                    if attempt < 3 {
                        tokio::time::sleep(Duration::from_millis(100 * attempt)).await;
                    }
                }
            }
        }
        anyhow::bail!(
            "Failed to delete source after copy ({}): {:#}",
            src.display(),
            last_err.unwrap()
        )
    }

/// Run the copy engine with multiple parallel workers
pub async fn run_copy_engine(
    files: Vec<(PathBuf, PathBuf)>,
    config: Arc<CopyConfig>,
    progress_sender: tokio::sync::mpsc::UnboundedSender<FileProgress>,
    cancel: Arc<AtomicBool>,
    pause: Option<Arc<AtomicBool>>,
    bytes_total: Arc<AtomicU64>,
    bytes_done: Arc<AtomicU64>,
) -> Result<()> {
    run_copy_engine_inner(files, config, progress_sender, cancel, pause, bytes_total, bytes_done, false).await
}

/// Run copy engine in move mode (deletes source after copy)
pub async fn run_move_engine(
    files: Vec<(PathBuf, PathBuf)>,
    config: Arc<CopyConfig>,
    progress_sender: tokio::sync::mpsc::UnboundedSender<FileProgress>,
    cancel: Arc<AtomicBool>,
    pause: Option<Arc<AtomicBool>>,
    bytes_total: Arc<AtomicU64>,
    bytes_done: Arc<AtomicU64>,
) -> Result<()> {
    run_copy_engine_inner(files, config, progress_sender, cancel, pause, bytes_total, bytes_done, true).await
}

/// ★ Lore-inspired: JoinSet-style parallel copy with severity-error handling
async fn run_copy_engine_inner(
    files: Vec<(PathBuf, PathBuf)>,
    config: Arc<CopyConfig>,
    progress_sender: tokio::sync::mpsc::UnboundedSender<FileProgress>,
    cancel: Arc<AtomicBool>,
    pause: Option<Arc<AtomicBool>>,
    bytes_total: Arc<AtomicU64>,
    bytes_done: Arc<AtomicU64>,
    move_mode: bool,
) -> Result<()> {
    let semaphore = Arc::new(Semaphore::new(config.threads));
    let mut handles = Vec::new();

    for (src, dst) in files {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .context("Failed to acquire semaphore permit")?;

        let config = config.clone();
        let sender = progress_sender.clone();
        let cancel = cancel.clone();
        let pause = pause.clone();
        let bt = bytes_total.clone();
        let bd = bytes_done.clone();

        handles.push(tokio::spawn(async move {
            let _permit = permit;
            let result = if move_mode {
                copy_file_with_move(&src, &dst, &config, sender, cancel, pause, bt, bd).await
            } else {
                copy_file(&src, &dst, &config, sender, cancel, pause, bt, bd).await
            };
            if let Err(e) = result {
                tracing::error!("Failed to copy {}: {:#}", src.display(), e);
            }
        }));
    }

    for h in handles {
        let _ = h.await;
    }

    Ok(())
}
use anyhow::{Context, Result};
use std::path::Path;
use tokio::io::AsyncReadExt;

/// Compute BLAKE3 hash of a file
pub async fn blake3_hash(path: &Path) -> Result<String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .context(format!("Failed to open: {}", path.display()))?;

    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

/// Compute XXH3-128 hash of a file
pub async fn xxh3_hash(path: &Path) -> Result<String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .context(format!("Failed to open: {}", path.display()))?;

    let mut hasher = xxhash_rust::xxh3::Xxh3::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.digest128().to_string())
}

/// Verify a copied file by comparing hashes
pub async fn verify_copy(src: &Path, dst: &Path, use_blake3: bool) -> Result<bool> {
    tracing::info!("Verifying: {} → {}", src.display(), dst.display());

    let (src_hash, dst_hash) = if use_blake3 {
        tokio::join!(blake3_hash(src), blake3_hash(dst))
    } else {
        tokio::join!(xxh3_hash(src), xxh3_hash(dst))
    };

    let src_hash = src_hash?;
    let dst_hash = dst_hash?;

    if src_hash == dst_hash {
        tracing::info!("✓ Hash match: {}", src_hash);
        Ok(true)
    } else {
        tracing::error!("✗ Hash mismatch: src={} dst={}", src_hash, dst_hash);
        Ok(false)
    }
}
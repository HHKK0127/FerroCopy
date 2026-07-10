use std::path::PathBuf;

/// Configuration for the copy engine
#[derive(Debug, Clone)]
pub struct CopyConfig {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub verify: bool,
    pub hash_algorithm: HashAlgorithm,
    pub threads: usize,
    pub recursive: bool,
    pub overwrite: OverwriteMode,
}

#[derive(Debug, Clone)]
pub enum HashAlgorithm {
    Blake3,
    Xxh3,
}

#[derive(Debug, Clone)]
pub enum OverwriteMode {
    Always,
    Skip,
    IfDifferent,
}

impl std::fmt::Display for HashAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Blake3 => write!(f, "BLAKE3"),
            Self::Xxh3 => write!(f, "XXH3-128"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileProgress {
    pub file: PathBuf,
    pub bytes_copied: u64,
    pub total_bytes: u64,
    pub speed_bytes_per_sec: f64,
}

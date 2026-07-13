use anyhow::Context;
use notify_debouncer_full::notify::Watcher;
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
    Prompt,
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

// ── Alacritty/Bevy-inspired: config hot-reload ───────────────────────

/// Persistent settings stored in TOML config file.
/// Bevy-inspired: single source of truth for user preferences.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct FerroConfig {
    pub verify: bool,
    pub hash_algorithm: String,
    pub threads: usize,
    pub overwrite: String,
    pub theme: String,
}

impl Default for FerroConfig {
    fn default() -> Self {
        Self {
            verify: false,
            hash_algorithm: "xxh3".to_string(),
            threads: num_cpus::get(),
            overwrite: "always".to_string(),
            theme: "cosmic".to_string(),
        }
    }
}

/// Config path: platform-specific config directory.
pub fn config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("ferrocopy").join("config.toml")
}

/// Load config from TOML file, or create default if missing.
pub fn load_config_file() -> FerroConfig {
    let path = config_path();
    if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(cfg) => return cfg,
                Err(e) => tracing::warn!("Config parse error (using defaults): {e}"),
            },
            Err(e) => tracing::warn!("Config read error (using defaults): {e}"),
        }
    }
    let cfg = FerroConfig::default();
    // Write default config if missing
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = toml::to_string_pretty(&cfg) {
        let _ = std::fs::write(&path, content);
    }
    cfg
}

/// Bevy-inspired: debounced file watcher for config hot-reload.
/// Uses `notify-debouncer-full` to batch rapid changes.
pub fn spawn_config_watcher<F>(on_change: F) -> anyhow::Result<()>
where
    F: Fn(FerroConfig) + Send + 'static,
{
    use notify_debouncer_full::new_debouncer;
    use std::sync::mpsc;
    use std::time::Duration;

    let path = config_path();
    if !path.exists() {
        tracing::info!("No config file at {:?}, watcher not started", path);
        return Ok(());
    }

    let (tx, rx) = mpsc::channel();
    let mut debouncer = new_debouncer(
        Duration::from_millis(500),
        None,
        tx,
    )
    .context("Failed to create config file watcher")?;

    debouncer
        .watch(
            path.parent().unwrap_or(std::path::Path::new(".")),
            notify_debouncer_full::notify::RecursiveMode::NonRecursive,
        )
        .context("Failed to watch config directory")?;

    std::thread::spawn(move || {
        for events in rx {
            match events {
                Ok(events) => {
                    let config_path = config_path();
                    let changed = events.iter().any(|e| {
                        e.paths.iter().any(|p| *p == config_path)
                    });
                    if changed {
                        let new_config = load_config_file();
                        tracing::info!("Config hot-reloaded");
                        on_change(new_config);
                    }
                }
                Err(e) => tracing::warn!("Config watch error: {e:?}"),
            }
        }
    });

    Ok(())
}

mod config;
mod engine;
mod gui;
mod hash;
mod shell;

use clap::Parser;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

/// FerroCopy — A high-performance file copy tool inspired by TeraCopy
#[derive(Parser)]
#[command(name = "ferrocopy", version = "0.1.0", about = "Fast parallel file copy with hash verification")]
struct Cli {
    /// Source file or directory (optional in GUI mode)
    source: Option<PathBuf>,

    /// Destination path (optional in GUI mode)
    destination: Option<PathBuf>,

    /// Number of parallel copy threads (default: number of CPU cores)
    #[arg(short = 'j', long, default_value_t = num_cpus::get())]
    threads: usize,

    /// Enable hash verification after copy
    #[arg(long)]
    verify: bool,

    /// Hash algorithm for verification [default: blake3]
    #[arg(long, default_value = "blake3", value_parser = clap::value_parser!(HashAlgorithm))]
    hash: HashAlgorithm,

    /// Recursively copy directories
    #[arg(short, long)]
    recursive: bool,

    /// Overwrite mode [always, skip, if-different]
    #[arg(long, default_value = "always")]
    overwrite: OverwriteModeArg,

    /// Verbose output
    #[arg(long)]
    verbose: bool,

    /// Launch in GUI mode
    #[arg(long)]
    gui: bool,

        /// Move mode: delete source after copy (CLI)
        #[arg(long)]
        move_files: bool,

        /// Install shell integration (right-click menu, Send To, paste handler)
        #[arg(long)]
        install_context: bool,

        /// Uninstall shell integration
        #[arg(long)]
        uninstall_context: bool,

        /// Shell copy action (invoked by right-click menu)
        #[arg(long)]
        shell_copy: bool,

        /// Shell move action (invoked by right-click menu)
        #[arg(long)]
        shell_move: bool,

        /// Shell paste action (invoked by folder background)
        #[arg(long)]
        shell_paste: bool,
    }

    #[derive(Clone, Debug)]
enum HashAlgorithm {
    Blake3,
    Xxh3,
}

impl std::str::FromStr for HashAlgorithm {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "blake3" => Ok(Self::Blake3),
            "xxh3" | "xxhash" => Ok(Self::Xxh3),
            _ => Err("Unsupported hash algorithm. Use 'blake3' or 'xxh3'.".into()),
        }
    }
}

#[derive(Clone, Debug)]
enum OverwriteModeArg {
    Always,
    Skip,
    IfDifferent,
}

impl std::str::FromStr for OverwriteModeArg {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "always" => Ok(Self::Always),
            "skip" => Ok(Self::Skip),
            "if-different" => Ok(Self::IfDifferent),
            _ => Err("Unsupported overwrite mode. Use 'always', 'skip', or 'if-different'.".into()),
        }
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // ── Shell integration commands ──
    if cli.install_context {
        return shell::install().map_err(|e| anyhow::anyhow!("{:#}", e));
    }
    if cli.uninstall_context {
        return shell::uninstall().map_err(|e| anyhow::anyhow!("{:#}", e));
    }

    // ── Shell action: copy from context menu ──
    if cli.shell_copy {
        let paths: Vec<String> = cli.source.iter().map(|p| p.display().to_string()).collect();
        return shell::handle_shell_copy(paths);
    }

    // ── Shell action: move from context menu ──
    if cli.shell_move {
        let paths: Vec<String> = cli.source.iter().map(|p| p.display().to_string()).collect();
        return shell::handle_shell_move(paths);
    }

    // ── Shell action: paste from folder background ──
    if cli.shell_paste {
        let dest = cli.source.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        return shell::handle_shell_paste(dest);
    }

    if cli.gui {
        tracing_subscriber::fmt().with_env_filter(EnvFilter::new("ferrocopy=error"))
            .with_target(false).with_line_number(false).init();
        gui::run_gui();
        return Ok(());
    }

    // Initialize logging (show only errors unless verbose)
    let filter = if cli.verbose {
        EnvFilter::new("ferrocopy=debug")
    } else {
        EnvFilter::new("ferrocopy=error")
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_line_number(false)
        .init();

    // Launch async runtime
    let rt = tokio::runtime::Runtime::new()?;
    if cli.source.is_none() || cli.destination.is_none() {
        anyhow::bail!("SOURCE and DESTINATION are required in CLI mode.");
    }
    rt.block_on(async_main(cli))?;

    Ok(())
}

async fn async_main(cli: Cli) -> anyhow::Result<()> {
    let config = Arc::new(config::CopyConfig {
        source: cli.source.clone().unwrap(),
        destination: cli.destination.clone().unwrap(),
        verify: cli.verify,
        hash_algorithm: match cli.hash {
            HashAlgorithm::Blake3 => config::HashAlgorithm::Blake3,
            HashAlgorithm::Xxh3 => config::HashAlgorithm::Xxh3,
        },
        threads: cli.threads,
        recursive: cli.recursive,
        overwrite: match cli.overwrite {
            OverwriteModeArg::Always => config::OverwriteMode::Always,
            OverwriteModeArg::Skip => config::OverwriteMode::Skip,
            OverwriteModeArg::IfDifferent => config::OverwriteMode::IfDifferent,
        },
    });

    let src = &config.source;
    let dst = &config.destination;

    if !src.exists() {
        anyhow::bail!("Source does not exist: {}", src.display());
    }

    println!(
        "FerroCopy v{} — {} → {}",
        env!("CARGO_PKG_VERSION"),
        src.display(),
        dst.display()
    );
    println!(
        "  Threads: {} | Verify: {} | Hash: {} | Recursive: {}",
        config.threads,
        if config.verify { "yes" } else { "no" },
        config.hash_algorithm,
        config.recursive,
    );
    println!();

    // Collect files
    let files = engine::collect_files(src, dst, config.recursive)?;
    if files.is_empty() {
        anyhow::bail!("No files to copy.");
    }
    let file_count = files.len();
    println!("  Found {} file(s) to copy.\n", file_count);

    // Set up progress channel
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<config::FileProgress>();

    let bytes_total = Arc::new(AtomicU64::new(0));
    let bytes_done = Arc::new(AtomicU64::new(0));
    let cancel = Arc::new(AtomicBool::new(false));

    // Progress bar
    let pb = indicatif::ProgressBar::new(file_count as u64);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files ({msg})")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message("initializing...");

    // Clone Arcs BEFORE spawning the progress task (to avoid move issues)
    let pb_bytes_done = bytes_done.clone();
    let pb_cancel = cancel.clone();
    tokio::spawn(async move {
        let start = std::time::Instant::now();
        let mut last_files: Vec<std::path::PathBuf> = Vec::new();
        while let Some(progress) = rx.recv().await {
            if pb_cancel.load(Ordering::Relaxed) {
                break;
            }
            // Track completed files for progress bar
            if progress.bytes_copied >= progress.total_bytes && progress.total_bytes > 0
                && !last_files.contains(&progress.file)
            {
                last_files.push(progress.file.clone());
                pb.inc(1);
            }
            let total_bytes = pb_bytes_done.load(Ordering::Relaxed);
            let elapsed = start.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 { total_bytes as f64 / elapsed } else { 0.0 };
            let speed_str = humansize::format_size(speed as u64, humansize::BINARY);
            pb.set_message(format!("{}/s - {}", speed_str, progress.file.display()));
        }
        pb.finish_with_message(format!(
            "Done! {} copied in {:.1}s",
            humansize::format_size(pb_bytes_done.load(Ordering::Relaxed), humansize::BINARY),
            start.elapsed().as_secs_f64(),
        ));
    });

    // Run copy engine
    let engine_config = config.clone();
    let engine_tx = tx.clone();
    let engine_cancel = cancel.clone();
    let engine_bt = bytes_total.clone();
    let engine_bd = bytes_done.clone();

    if cli.move_files {
        engine::run_move_engine(files, engine_config, engine_tx, engine_cancel, None, engine_bt, engine_bd).await?;
        println!("✓ Files moved (source deleted).");
    } else {
        engine::run_copy_engine(files, engine_config, engine_tx, engine_cancel, None, engine_bt, engine_bd).await?;
    }

    // Wait a moment for progress display to update
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let total = bytes_done.load(Ordering::Relaxed);
    println!(
        "\n✓ Copy complete: {} files, {} total",
        file_count,
        humansize::format_size(total, humansize::BINARY),
    );

    if config.verify {
        println!("✓ All files verified.");
    }

    Ok(())
}

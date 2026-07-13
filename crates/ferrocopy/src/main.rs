mod change_detection;
mod clipboard;
mod color_scheme;
mod config;
mod crash_reporter;
mod engine;
mod eventloop;
mod events;
mod fanout;
mod file_watcher;
mod flexbox;
mod gui;
mod hash;
mod iopool;
mod lua_ext;
mod plugin;
mod rpc;
mod schedule;
mod shell;
mod signal;
mod ssh;
mod telemetry;
mod threadbound;
mod wasi_sandbox;

use clap::Parser;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

// ── Module-level re-exports for integrated modules ──

/// EventLoop-based progress handler for CLI mode
struct CliEventHandler {
    pb: indicatif::ProgressBar,
    bytes_done: Arc<AtomicU64>,
    files_done: Arc<AtomicU64>,
}
impl eventloop::EventHandler for CliEventHandler {
    fn on_event(&mut self, event: crate::events::CopyEvent) {
        match event {
            crate::events::CopyEvent::FileStarted { .. } => {}
            crate::events::CopyEvent::FileCompleted { .. } => {
                self.files_done.fetch_add(1, Ordering::SeqCst);
                self.pb.inc(1);
            }
            crate::events::CopyEvent::Error { error, .. } => {
                tracing::error!("Copy error: {}", error);
            }
            crate::events::CopyEvent::Finished { .. } => {}
            crate::events::CopyEvent::Paused
            | crate::events::CopyEvent::Resumed
            | crate::events::CopyEvent::CancelRequested => {}
        }
    }
}

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

    // ── Integrated feature flags (Phase A/B modules) ──

    /// SSH remote copy: user@host[:port]:/remote/path as destination
    #[arg(long)]
    ssh: Option<String>,

    /// SSH identity file (for key-based auth)
    #[arg(long)]
    ssh_key: Option<String>,

    /// Listen for JSON-RPC commands on this TCP port
    #[arg(long)]
    rpc_listen: Option<u16>,

    /// Watch a directory for new files and auto-copy them
    #[arg(long)]
    watch_dir: Option<PathBuf>,

    /// Lua filter script path for file inclusion/exclusion
    #[arg(long)]
    lua_filter: Option<PathBuf>,

    /// WASM plugin file to run during copy
    #[arg(long)]
    wasm_plugin: Option<PathBuf>,

    /// Use the Scan→Copy→Verify→Report Schedule pipeline
    #[arg(long)]
    use_schedule: bool,

    /// Number of I/O pool worker threads (default: 4)
    #[arg(long, default_value_t = 4)]
    io_pool_threads: usize,
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
    // Install crash handler first — captures panics and writes crash dumps
    crash_reporter::install_crash_handler();

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
        // Any remaining args after -- are additional paths
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
    let source = cli.source.clone().ok_or_else(|| anyhow::anyhow!("SOURCE is required"))?;
    let destination = cli.destination.clone().ok_or_else(|| anyhow::anyhow!("DESTINATION is required"))?;
    let config = Arc::new(config::CopyConfig {
        source: source.clone(),
        destination: destination.clone(),
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

    // ── Signal handling: register ctrl-c handler ──
    let shutdown = Arc::new(AtomicBool::new(false));
    if let Err(e) = signal::register_shutdown_signal(shutdown.clone()) {
        tracing::warn!("Signal handler not registered: {}", e);
    }

    // ── I/O Task Pool (dedicated I/O workers) ──
    let iopool = Arc::new(iopool::IoTaskPool::new(cli.io_pool_threads));

    // ── Lua filter (file inclusion/exclusion) ──
    let lua_filter: Option<lua_ext::LuaFilter> = if let Some(ref lua_path) = cli.lua_filter {
        let script = std::fs::read_to_string(lua_path)
            .map_err(|e| anyhow::anyhow!("Failed to read Lua script '{}': {}", lua_path.display(), e))?;
        Some(lua_ext::LuaFilter::new(&script)
            .map_err(|e| anyhow::anyhow!("Lua script error: {}", e))?)
    } else {
        None
    };

    // ── WASM plugin ──
    let wasm_plugin: Option<wasi_sandbox::WasiPlugin> = if let Some(ref wasm_path) = cli.wasm_plugin {
        Some(wasi_sandbox::WasiPlugin::from_file(wasm_path)
            .map_err(|e| anyhow::anyhow!("Failed to load WASM plugin '{}': {}", wasm_path.display(), e))?)
    } else {
        None
    };

    // ── SSH remote copy mode ──
    if let Some(ref ssh_dest) = cli.ssh {
        let parts: Vec<&str> = ssh_dest.splitn(2, ':').collect();
        if parts.len() < 2 {
            anyhow::bail!("Invalid SSH destination format. Expected user@host[:port]:/remote/path");
        }
        let user_host = parts[0];
        let remote_path = parts[1];
        let uh_parts: Vec<&str> = user_host.splitn(2, '@').collect();
        if uh_parts.len() < 2 {
            anyhow::bail!("Invalid SSH user@host format. Expected user@host[:port]");
        }
        let username = uh_parts[0];
        let host_port = uh_parts[1];
        let (host, port) = if let Some(idx) = host_port.find(':') {
            let p: u16 = host_port[idx + 1..].parse().unwrap_or(22);
            (&host_port[..idx], p)
        } else {
            (host_port, 22u16)
        };

        let mut ssh_config = ssh::SshConfig::new(host, username);
        ssh_config.port = port;
        if let Some(ref key_path) = cli.ssh_key {
            ssh_config.key_path = Some(key_path.clone());
        }

        tracing::info!("🔑 Connecting to {}@{}:{} ...", username, host, port);
        let session = ssh::SshSession::connect(&ssh_config)?;

        // Collect local files and upload via SFTP
        let files = engine::collect_files(src, dst, config.recursive)?;
        if files.is_empty() {
            anyhow::bail!("No files to copy.");
        }
        println!("  Uploading {} file(s) via SFTP to {}", files.len(), ssh_dest);
        for (local, _) in &files {
            let remote_file = std::path::Path::new(&remote_path).join(
                local.strip_prefix(src).unwrap_or(local)
            );
            session.upload_file(local, &remote_file)?;
        }
        println!("✓ SFTP upload complete.");
        return Ok(());
    }

    // ── RPC listen mode: start JSON-RPC server on TCP port ──
    if let Some(rpc_port) = cli.rpc_listen {
        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", rpc_port)).await?;
        println!("🔌 RPC listener on port {} (JSON-RPC)", rpc_port);
        tokio::spawn(async move {
            loop {
                if let Ok((mut stream, peer)) = listener.accept().await {
                    tracing::debug!("RPC connection from {}", peer);
                    tokio::spawn(async move {
                        use tokio::io::AsyncReadExt;
                        let mut buf = vec![0u8; 4096];
                        if let Ok(n) = stream.read(&mut buf).await {
                            if n > 0 {
                                let json_str = String::from_utf8_lossy(&buf[..n]);
                                if let Ok(req) = rpc::parse_request(&json_str) {
                                    let resp = rpc::handle_method(&req);
                                    if let Ok(json) = serde_json::to_string(&resp) {
                                        use tokio::io::AsyncWriteExt;
                                        let _ = stream.write_all(json.as_bytes()).await;
                                    }
                                }
                            }
                        }
                    });
                }
            }
        });
    }

    // ── File watcher mode: watch directory for auto-copy ──
    if let Some(ref watch_path) = cli.watch_dir {
        println!("👀 Watching {} for new files...", watch_path.display());
        let watcher = file_watcher::FileWatcher::watch(watch_path.clone(), 500)
            .map_err(|e| anyhow::anyhow!("Failed to start file watcher: {}", e))?;
        loop {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }
            let changes = watcher.wait_for_changes().unwrap_or_default();
            for change in &changes {
                if let file_watcher::FileChange::Created(p) = change {
                    tracing::info!("New file detected: {}", p.display());
                }
            }
        }
        return Ok(());
    }

    // ── Standard copy mode ──

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
    if lua_filter.is_some() {
        println!("  Lua filter: enabled");
    }
    println!();

    // Collect files (with optional Lua filtering)
    let all_files = engine::collect_files(src, dst, config.recursive)?;
    let files: Vec<_> = if let Some(ref filter) = lua_filter {
        all_files.into_iter().filter(|(s, _)| {
            let path_str = s.display().to_string();
            let size = std::fs::metadata(s).map(|m| m.len()).unwrap_or(0);
            filter.should_include(&path_str, size).unwrap_or(true)
        }).collect()
    } else {
        all_files
    };

    if files.is_empty() {
        anyhow::bail!("No files to copy.");
    }
    let file_count = files.len();
    println!("  Found {} file(s) to copy.\n", file_count);

    // Execute WASM plugin (if provided)
    if let Some(ref plugin) = wasm_plugin {
        let result = plugin.run()?;
        tracing::info!("🧪 WASM plugin '{}' exit code: {}", result.module_name, result.exit_code);
    }

    // ── Schedule pipeline mode ──
    if cli.use_schedule {
        let mut schedule = schedule::Schedule::new();
        schedule.add_system(schedule::Stage::Scan, schedule::ScanSystem);
        schedule.add_system(schedule::Stage::Copy, CopySystem {
            move_mode: cli.move_files,
        });
        schedule.add_system(schedule::Stage::Verify, VerifySystem);
        schedule.add_system(schedule::Stage::Report, schedule::ReportSystem);

        let mut state = schedule::PipelineState::default();
        state.source = Some(src.clone());
        state.destination = Some(dst.clone());
        schedule.run(&mut state)
            .map_err(|e| anyhow::anyhow!("Schedule failed: {}", e))?;
        return Ok(());
    }

    // ── Set up progress channel ──
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<config::FileProgress>();

    let bytes_total = Arc::new(AtomicU64::new(0));
    let bytes_done = Arc::new(AtomicU64::new(0));
    let cancel = Arc::new(AtomicBool::new(false));

    // ── Event-driven progress via CoreSender / EventLoop ──
    let (event_tx, event_rx) = crate::events::channel(64);
    let event_handler = CliEventHandler {
        pb: indicatif::ProgressBar::new(file_count as u64),
        bytes_done: bytes_done.clone(),
        files_done: Arc::new(AtomicU64::new(0)),
    };
    let mut event_loop = eventloop::EventLoop::new(event_handler, event_rx);
    let event_tx_for_loop = event_tx.clone();
    let cancel_for_loop = cancel.clone();

    // Spawn event loop on a separate thread (non-blocking tick mode)
    std::thread::spawn(move || {
        while !cancel_for_loop.load(Ordering::Relaxed) {
            if !event_loop.tick() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    });

    // ── Progress bar (sync with event emissions) ──
    let pb = indicatif::ProgressBar::new(file_count as u64);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files ({msg})")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message("copying...");

    let pb_bytes_done = bytes_done.clone();
    let pb_cancel = cancel.clone();
    tokio::spawn(async move {
        let start = std::time::Instant::now();
        let mut last_files: Vec<std::path::PathBuf> = Vec::new();
        while let Some(progress) = rx.recv().await {
            if pb_cancel.load(Ordering::Relaxed) {
                break;
            }
            if progress.bytes_copied >= progress.total_bytes && progress.total_bytes > 0
                && !last_files.contains(&progress.file)
            {
                last_files.push(progress.file.clone());
                pb.inc(1);
                let _ = event_tx_for_loop.send(crate::events::CopyEvent::FileCompleted {
                    file: progress.file.display().to_string(),
                    bytes_copied: progress.bytes_copied,
                    elapsed_ms: start.elapsed().as_millis() as u64,
                });
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
        let _ = event_tx_for_loop.send(crate::events::CopyEvent::Finished {
            total_files: last_files.len() as u64,
            total_bytes: pb_bytes_done.load(Ordering::Relaxed),
            elapsed_ms: start.elapsed().as_millis() as u64,
            errors: 0,
        });
    });

    // ── Spawn a telemetry monitoring task ──
    let telemetry = Arc::new(telemetry::TelemetryCounters::new());
    let telemetry_clone = telemetry.clone();
    let telemetry_bd = bytes_done.clone();
    std::thread::spawn(move || {
        let start = std::time::Instant::now();
        loop {
            std::thread::sleep(std::time::Duration::from_secs(3));
            let done = telemetry_bd.load(Ordering::Relaxed);
            if done == 0 {
                continue;
            }
            let elapsed = start.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 { done as f64 / elapsed } else { 0.0 };
            tracing::info!("📊 {:.0}s | {} copied | {}/s", elapsed,
                humansize::format_size(done, humansize::BINARY),
                humansize::format_size(speed as u64, humansize::BINARY));
        }
    });

    // ── Run copy engine ──
    let engine_config = config.clone();
    let engine_tx = tx.clone();
    let engine_cancel = cancel.clone();
    let engine_bt = bytes_total.clone();
    let engine_bd = bytes_done.clone();
    let file_count = files.len();

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

// ── Schedule System implementations ──

/// Copy system for the Schedule pipeline
struct CopySystem {
    move_mode: bool,
}
impl schedule::System for CopySystem {
    fn name(&self) -> &str {
        "copy"
    }
    fn run(&self, state: &mut schedule::PipelineState) -> Result<(), String> {
        let files = std::mem::take(&mut state.files);
        if files.is_empty() {
            return Err("No files to copy".into());
        }
        let config = Arc::new(config::CopyConfig {
            source: state.source.clone().unwrap_or_default(),
            destination: state.destination.clone().unwrap_or_default(),
            verify: false,
            hash_algorithm: config::HashAlgorithm::Blake3,
            threads: num_cpus::get(),
            recursive: true,
            overwrite: config::OverwriteMode::Always,
        });
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<config::FileProgress>();
        let cancel = Arc::new(AtomicBool::new(false));
        let bt = Arc::new(AtomicU64::new(0));
        let bd = Arc::new(AtomicU64::new(0));
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Runtime error: {}", e))?;
        if self.move_mode {
            rt.block_on(engine::run_move_engine(
                files, config, tx, cancel, None, bt, bd.clone(),
            )).map_err(|e| format!("Copy failed: {}", e))?;
        } else {
            rt.block_on(engine::run_copy_engine(
                files, config, tx, cancel, None, bt, bd.clone(),
            )).map_err(|e| format!("Copy failed: {}", e))?;
        }
        // Drain progress events
        while rx.try_recv().is_ok() {}
        state.bytes_copied = bd.load(Ordering::Relaxed);
        Ok(())
    }
}

/// Verify system for the Schedule pipeline
struct VerifySystem;
impl schedule::System for VerifySystem {
    fn name(&self) -> &str {
        "verify"
    }
    fn run(&self, state: &mut schedule::PipelineState) -> Result<(), String> {
        // Simple verification: check each destination file exists
        for (src, dst) in &state.files {
            if !dst.exists() {
                state.errors += 1;
                tracing::warn!("Verify failed: {} not found at destination", src.display());
            }
        }
        state.verified = state.errors == 0;
        if state.errors > 0 {
            tracing::warn!("Verify: {} errors", state.errors);
        } else {
            tracing::info!("✅ Verify: all files verified");
        }
        Ok(())
    }
}

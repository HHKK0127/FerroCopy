//! FerroCopy GUI — egui-based interface for the file copy engine
//!
//! ZED-inspired features:
//!   - StatusBar: bottom bar with speed/elapsed/ETA/file count (cf. ZED status_bar.rs)
//!   - Toast notifications: non-intrusive auto-dismiss popups (cf. ZED toast_layer.rs)
//!   - Pause/Resume: task lifecycle management (cf. ZED tasks.rs)

use crate::config::*;
use crate::engine;
use eframe::egui;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::sync::mpsc as std_mpsc;
use std::time::Instant;

// ── Public entry point ───────────────────────────────────────────────

/// Launch the egui GUI
pub fn run_gui() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_resizable(true)
            .with_title("FerroCopy"),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "FerroCopy",
        options,
        Box::new(|_cc| Ok(Box::new(FerroCopyApp::new()) as Box<dyn eframe::App>)),
    );
}

// ── State types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum AppStatus {
    Idle,
    Scanning,
    Copying,
    Paused,
    Done,
    Error,
}

#[derive(Debug, Clone)]
struct FileEntry {
    name: String,
    size: u64,
    progress: f32,
    status: String,
}

/// Commands the GUI sends to the engine worker thread
enum EngineCommand {
    Start {
        src: PathBuf,
        dst: PathBuf,
        recursive: bool,
        verify: bool,
        hash_algo: HashAlgorithm,
        threads: usize,
    },
    Cancel,
    Pause,
    Resume,
}

// ── ZED-inspired Toast notification ──────────────────────────────────
/// Refs: ZED toast_layer.rs — auto-dismiss timer with pause/resume on hover

#[derive(Debug, Clone)]
struct ToastNotification {
    id: u64,
    message: String,
    toast_type: ToastType,
    remaining: f32, // seconds until auto-dismiss
}

static NEXT_TOAST_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq)]
enum ToastType {
    Info,
    Success,
    Error,
}

impl ToastNotification {
    fn new(message: impl Into<String>, toast_type: ToastType) -> Self {
        let id = NEXT_TOAST_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            id,
            message: message.into(),
            toast_type,
            remaining: 5.0, // auto-dismiss after 5 seconds
        }
    }
}

/// State shared between GUI thread and engine worker
struct GuiState {
    source: String,
    destination: String,
    recursive: bool,
    verify: bool,
    hash_algorithm: HashAlgorithm,
    threads: usize,
    status: AppStatus,
    files: Vec<FileEntry>,
    current_file: String,
    speed: f64,
    files_done: usize,
    error_message: String,
    log: Vec<String>,
    show_about: bool,
    // ZED-inspired: toast notifications
    toasts: Vec<ToastNotification>,
    // ZED-inspired: status bar items
    elapsed_secs: f64,
    eta_secs: f64,
}

impl Default for GuiState {
    fn default() -> Self {
        Self {
            source: String::new(),
            destination: String::new(),
            recursive: false,
            verify: false,
            hash_algorithm: HashAlgorithm::Blake3,
            threads: num_cpus::get(),
            status: AppStatus::Idle,
            files: Vec::new(),
            current_file: String::new(),
            speed: 0.0,
            files_done: 0,
            error_message: String::new(),
            log: Vec::new(),
            show_about: false,
            toasts: Vec::new(),
            elapsed_secs: 0.0,
            eta_secs: 0.0,
        }
    }
}

// ── The egui app ─────────────────────────────────────────────────────

pub struct FerroCopyApp {
    state: Arc<Mutex<GuiState>>,
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
    started_at: Arc<Mutex<Option<Instant>>>,
    bytes_total: Arc<AtomicU64>,
    bytes_done: Arc<AtomicU64>,
    /// Commands to the engine worker thread
    cmd_tx: std_mpsc::Sender<EngineCommand>,
    /// Progress updates from the engine
    progress_rx: Arc<Mutex<Option<std_mpsc::Receiver<ProgressUpdate>>>>,
}

#[derive(Debug, Clone)]
struct ProgressUpdate {
    file: String,
    bytes_copied: u64,
    total_bytes: u64,
    speed: f64,
    is_done: bool,
    error: String,
    log: Vec<String>,
}

impl FerroCopyApp {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = std_mpsc::channel::<EngineCommand>();
        let (progress_tx, progress_rx) = std_mpsc::channel::<ProgressUpdate>();

        std::thread::spawn(move || {
            engine_worker(cmd_rx, progress_tx);
        });

        Self {
            state: Arc::new(Mutex::new(GuiState::default())),
            cancel: Arc::new(AtomicBool::new(false)),
            pause: Arc::new(AtomicBool::new(false)),
            started_at: Arc::new(Mutex::new(None)),
            bytes_total: Arc::new(AtomicU64::new(0)),
            bytes_done: Arc::new(AtomicU64::new(0)),
            cmd_tx,
            progress_rx: Arc::new(Mutex::new(Some(progress_rx))),
        }
    }
}

impl Default for FerroCopyApp {
    fn default() -> Self {
        Self::new()
    }
}

impl eframe::App for FerroCopyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Drain any pending progress updates
        if let Ok(rx_lock) = self.progress_rx.lock() {
            if let Some(rx) = rx_lock.as_ref() {
                while let Ok(p) = rx.try_recv() {
                    self.apply_progress(p);
                }
            }
        }

        // Repaint frequently during copy / pause
        let status = self.state.lock().unwrap().status.clone();
        let needs_repaint = matches!(status, AppStatus::Copying | AppStatus::Paused);
        if needs_repaint {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }

        // ── Top bar ──
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.heading("⚡ FerroCopy");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("About").clicked() {
                        self.state.lock().unwrap().show_about = true;
                    }
                });
            });
        });

        // ── ZED-inspired: Toast notifications overlay ──
        // Rendered as floating windows in top-right; auto-dismiss timer
        self.show_toasts(ctx);

        // ── About window ──
        if self.state.lock().unwrap().show_about {
            let mut open = true;
            egui::Window::new("About FerroCopy")
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.heading("FerroCopy v0.1.0");
                    ui.label("A high-performance file copy tool");
                    ui.label("Inspired by TeraCopy — Built with Rust + egui");
                    ui.separator();
                    ui.label("Features:");
                    ui.label("  • Parallel file copy (adaptive buffer)");
                    ui.label("  • BLAKE3 / XXH3-128 hash verification");
                    ui.label("  • Recursive directory copy");
                    ui.label("  • Full timestamp preservation");
                    ui.label("  • Pause / Resume");
                    ui.label("  • ZED-inspired StatusBar + Toast notifications");
                });
            if !open {
                self.state.lock().unwrap().show_about = false;
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let mut state = self.state.lock().unwrap();

            // ── Status ──
            let status_text = match &state.status {
                AppStatus::Idle => "Ready",
                AppStatus::Scanning => "Scanning files...",
                AppStatus::Copying => "Copying...",
                AppStatus::Paused => "⏸ Paused",
                AppStatus::Done => "✓ Complete",
                AppStatus::Error => "✗ Error",
            };
            ui.horizontal(|ui| {
                let color = match &state.status {
                    AppStatus::Paused => egui::Color32::YELLOW,
                    AppStatus::Error => egui::Color32::RED,
                    AppStatus::Done => egui::Color32::GREEN,
                    _ => egui::Color32::WHITE,
                };
                ui.colored_label(color, format!("Status: {}", status_text));
                if !state.error_message.is_empty() {
                    ui.colored_label(egui::Color32::RED, &state.error_message);
                }
            });
            ui.separator();

            // ── Source ──
            ui.horizontal(|ui| {
                ui.label("Source:");
                let mut src = state.source.clone();
                if ui.add(egui::TextEdit::singleline(&mut src).desired_width(400.0)).changed() {
                    state.source = src;
                }
                if ui.button("📁").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Select source file or directory")
                        .pick_file()
                    {
                        state.source = path.display().to_string();
                    }
                }
            });

            // ── Destination ──
            ui.horizontal(|ui| {
                ui.label("Dest:");
                let mut dst = state.destination.clone();
                if ui.add(egui::TextEdit::singleline(&mut dst).desired_width(400.0)).changed() {
                    state.destination = dst;
                }
                if ui.button("📁").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Select destination directory")
                        .pick_folder()
                    {
                        state.destination = path.display().to_string();
                    }
                }
            });

            ui.separator();

            // ── Options ──
            ui.horizontal(|ui| {
                ui.checkbox(&mut state.recursive, "Recursive");
                ui.checkbox(&mut state.verify, "Verify hash");
            });
            ui.horizontal(|ui| {
                ui.label("Hash:");
                let is_blake3 = matches!(state.hash_algorithm, HashAlgorithm::Blake3);
                egui::ComboBox::from_id_salt("hash_choice")
                    .selected_text(if is_blake3 { "BLAKE3" } else { "XXH3-128" })
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(is_blake3, "BLAKE3").clicked() {
                            state.hash_algorithm = HashAlgorithm::Blake3;
                        }
                        if ui.selectable_label(!is_blake3, "XXH3-128").clicked() {
                            state.hash_algorithm = HashAlgorithm::Xxh3;
                        }
                    });
                ui.label(format!("  Threads: {}", state.threads));
            });

            ui.separator();

            // ── Action buttons ──
            let can_start = !state.source.is_empty()
                && !state.destination.is_empty()
                && state.status == AppStatus::Idle;
            let is_copying = state.status == AppStatus::Copying;
            let is_paused = state.status == AppStatus::Paused;

            let mut start_clicked: bool = false;
            let mut pause_clicked: bool = false;
            let mut resume_clicked: bool = false;
            let mut cancel_clicked: bool = false;
            let mut clear_clicked: bool = false;
            ui.horizontal(|ui| {
                start_clicked = ui.add_enabled(can_start, egui::Button::new("▶ Start")).clicked();
                pause_clicked = ui.add_enabled(is_copying, egui::Button::new("⏸ Pause")).clicked();
                resume_clicked = ui.add_enabled(is_paused, egui::Button::new("▶ Resume")).clicked();
                cancel_clicked = ui
                    .add_enabled(is_copying || is_paused, egui::Button::new("⏹ Cancel"))
                    .clicked();
                let show_clear = matches!(state.status, AppStatus::Done | AppStatus::Error);
                clear_clicked = show_clear && ui.button("Clear").clicked();
            });

            // Snapshot values to use after dropping the lock
            let start_args: Option<(PathBuf, PathBuf, bool, bool, HashAlgorithm, usize)> = if start_clicked {
                Some((
                    PathBuf::from(&state.source),
                    PathBuf::from(&state.destination),
                    state.recursive,
                    state.verify,
                    state.hash_algorithm.clone(),
                    state.threads,
                ))
            } else {
                None
            };

            if pause_clicked {
                self.pause.store(true, Ordering::SeqCst);
                let _ = self.cmd_tx.send(EngineCommand::Pause);
                state.status = AppStatus::Paused;
            }
            if resume_clicked {
                self.pause.store(false, Ordering::SeqCst);
                let _ = self.cmd_tx.send(EngineCommand::Resume);
                state.status = AppStatus::Copying;
            }
            if cancel_clicked {
                self.cancel.store(true, Ordering::SeqCst);
                self.pause.store(false, Ordering::SeqCst);
                let _ = self.cmd_tx.send(EngineCommand::Cancel);
                state.log.push("Cancelled by user".to_string());
                state.status = AppStatus::Idle;
            }
            if clear_clicked {
                *state = GuiState::default();
                self.cancel.store(false, Ordering::SeqCst);
                self.pause.store(false, Ordering::SeqCst);
                self.bytes_total.store(0, Ordering::SeqCst);
                self.bytes_done.store(0, Ordering::SeqCst);
                if let Ok(mut started) = self.started_at.lock() {
                    *started = None;
                }
                return;
            }

            if start_clicked {
                state.status = AppStatus::Scanning;
                state.current_file.clear();
                state.speed = 0.0;
                state.files_done = 0;
                state.error_message.clear();
                state.log.clear();
                state.files.clear();
                self.cancel.store(false, Ordering::SeqCst);
                self.pause.store(false, Ordering::SeqCst);
                self.bytes_total.store(0, Ordering::SeqCst);
                self.bytes_done.store(0, Ordering::SeqCst);
                if let Ok(mut started) = self.started_at.lock() {
                    *started = Some(Instant::now());
                }
            }

            drop(state);

            if let Some((src, dst, recursive, verify, hash_algo, threads)) = start_args {
                let _ = self.cmd_tx.send(EngineCommand::Start {
                    src, dst, recursive, verify, hash_algo, threads,
                });
            }

            // ── File list ──
            let state = self.state.lock().unwrap();
            if !state.files.is_empty() {
                ui.separator();
                ui.label(format!("Files ({}):", state.files.len()));
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for entry in &state.files {
                            ui.horizontal(|ui| {
                                ui.label(&entry.name);
                                ui.add(
                                    egui::ProgressBar::new(entry.progress)
                                        .desired_width(120.0),
                                );
                                ui.label(&entry.status);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(humansize::format_size(
                                            entry.size,
                                            humansize::BINARY,
                                        ));
                                    },
                                );
                            });
                        }
                    });
            }

            // ── Total progress ──
            if matches!(state.status, AppStatus::Copying | AppStatus::Paused | AppStatus::Done) {
                ui.separator();
                let total = self.bytes_total.load(Ordering::Relaxed);
                let done = self.bytes_done.load(Ordering::Relaxed);
                if total > 0 {
                    ui.label(format!(
                        "{} / {}",
                        humansize::format_size(done, humansize::BINARY),
                        humansize::format_size(total, humansize::BINARY),
                    ));
                    let ratio = done as f64 / total as f64;
                    let pb_color = if state.status == AppStatus::Paused {
                        egui::Color32::YELLOW
                    } else {
                        egui::Color32::from_rgb(80, 180, 255)
                    };
                    ui.add(
                        egui::ProgressBar::new(ratio as f32)
                            .text(format!("{:.1}%", ratio * 100.0))
                            .fill(pb_color),
                    );
                }
                if !state.current_file.is_empty() {
                    ui.label(format!("File: {}", state.current_file));
                }
                if state.speed > 0.0 {
                    ui.label(format!(
                        "Speed: {}/s",
                        humansize::format_size(state.speed as u64, humansize::BINARY),
                    ));
                }
                if !state.files.is_empty() {
                    ui.label(format!("Files: {}/{}", state.files_done, state.files.len()));
                }
            }

            // ── Log ──
            if !state.log.is_empty() {
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(100.0)
                    .show(ui, |ui| {
                        for msg in &state.log {
                            ui.label(msg);
                        }
                    });
            }
        });

        // ── ZED-inspired: Bottom StatusBar ──
        // cf. ZED status_bar.rs — left/right item groups layout
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Left group: status + current file
                let status = self.state.lock().unwrap().status.clone();
                let file = self.state.lock().unwrap().current_file.clone();
                let done = self.bytes_done.load(Ordering::Relaxed);
                let total = self.bytes_total.load(Ordering::Relaxed);

                let icon = match &status {
                    AppStatus::Idle => "⚡",
                    AppStatus::Scanning => "🔍",
                    AppStatus::Copying => "📋",
                    AppStatus::Paused => "⏸",
                    AppStatus::Done => "✅",
                    AppStatus::Error => "❌",
                };
                ui.label(icon);
                let status_str = match &status {
                    AppStatus::Idle => "Ready".into(),
                    AppStatus::Scanning => "Scanning...".into(),
                    AppStatus::Copying => format!("📄 {}", &file),
                    AppStatus::Paused => "Paused".into(),
                    AppStatus::Done => format!("{} copied", humansize::format_size(done, humansize::BINARY)),
                    AppStatus::Error => "Error".into(),
                };
                ui.label(&status_str);

                // Right group (pushed to the right)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let elapsed = self.state.lock().unwrap().elapsed_secs;
                    if elapsed > 0.0 {
                        ui.label(format!("⏱ {:.0}s", elapsed));
                    }
                    let eta = self.state.lock().unwrap().eta_secs;
                    if eta > 0.0 && eta.is_finite() {
                        ui.label(format!("⏳ {:.0}s", eta));
                    }
                    if total > 0 {
                        let pct = done as f64 / total as f64 * 100.0;
                        ui.label(format!("📊 {:.0}%", pct));
                    }
                    let speed = self.state.lock().unwrap().speed;
                    if speed > 0.0 {
                        ui.label(format!(
                            "🚀 {}/s",
                            humansize::format_size(speed as u64, humansize::BINARY),
                        ));
                    }
                });
            });
        });
    }
}

impl FerroCopyApp {
    fn apply_progress(&self, p: ProgressUpdate) {
        if let Ok(mut state) = self.state.lock() {
            state.current_file = p.file.clone();
            state.speed = p.speed;

            // Update elapsed time
            if let Ok(started) = self.started_at.lock() {
                if let Some(start) = *started {
                    state.elapsed_secs = start.elapsed().as_secs_f64();
                }
            }

            if p.is_done {
                state.status = if p.error.is_empty() {
                    AppStatus::Done
                } else {
                    state.error_message = p.error.clone();
                    AppStatus::Error
                };
                // ZED-inspired: add toast on completion
                let files_done = state.files_done;
                if p.error.is_empty() {
                    state.toasts.push(ToastNotification::new(
                        format!("✅ Copy complete! {} files copied", files_done),
                        ToastType::Success,
                    ));
                } else {
                    state.toasts.push(ToastNotification::new(
                        format!("❌ {}", p.error),
                        ToastType::Error,
                    ));
                }
                for msg in &p.log {
                    state.log.push(msg.clone());
                }
            } else if let Some(entry) = state.files.iter_mut().find(|e| e.name == p.file) {
                if p.total_bytes > 0 {
                    entry.progress = p.bytes_copied as f32 / p.total_bytes as f32;
                }
                if p.bytes_copied >= p.total_bytes && p.total_bytes > 0 {
                    entry.status = "Done".to_string();
                    state.files_done += 1;
                } else if entry.status == "Pending" {
                    entry.status = "Copying".to_string();
                }
            }
            for msg in &p.log {
                state.log.push(msg.clone());
            }

            // Update ETA estimation
            let done = self.bytes_done.load(Ordering::Relaxed);
            let total = self.bytes_total.load(Ordering::Relaxed);
            if done > 0 && total > done && state.elapsed_secs > 1.0 {
                let speed_bps = done as f64 / state.elapsed_secs;
                let remaining = (total - done) as f64;
                state.eta_secs = remaining / speed_bps;
            }
        }
    }

    /// ZED-inspired toast overlay with auto-dismiss
    /// cf. ZED toast_layer.rs — auto-dismiss timer
    fn show_toasts(&self, ctx: &egui::Context) {
        let mut state = self.state.lock().unwrap();
        if state.toasts.is_empty() {
            return;
        }

        // Update auto-dismiss timers
        let dt = ctx.input(|i| i.unstable_dt) as f32;
        state.toasts.retain_mut(|t| {
            t.remaining -= dt;
            t.remaining > 0.0
        });

        let toasts = state.toasts.clone();
        drop(state);

        // Show each toast as a floating window in top-right area
        let screen_rect = ctx.screen_rect();
        let mut y_offset = 10.0;

        for toast in &toasts {
            let color = match toast.toast_type {
                ToastType::Success => egui::Color32::from_rgb(76, 175, 80),
                ToastType::Error => egui::Color32::from_rgb(244, 67, 54),
                ToastType::Info => egui::Color32::from_rgb(33, 150, 243),
            };

            let id = egui::Id::new("toast").with(toast.id);
            let area = egui::Area::new(id)
                .fixed_pos(egui::pos2(screen_rect.right() - 310.0, y_offset))
                .order(egui::Order::Foreground);

            area.show(ctx, |ui| {
                let frame = egui::Frame::none()
                    .fill(egui::Color32::from_black_alpha(220))
                    .rounding(egui::Rounding::same(6.0))
                    .stroke(egui::Stroke::new(1.0, color));
                frame.show(ui, |ui| {
                    ui.set_max_width(280.0);
                    ui.horizontal(|ui| {
                        let icon = match toast.toast_type {
                            ToastType::Success => "✅",
                            ToastType::Error => "❌",
                            ToastType::Info => "ℹ️",
                        };
                        ui.label(egui::RichText::new(icon).color(color));
                        ui.label(egui::RichText::new(&toast.message).size(13.0));
                    });
                });
            });

            y_offset += 50.0;
        }
    }
}

// ── Engine worker thread ─────────────────────────────────────────────

/// Runs in a background thread, receives commands from the GUI,
/// executes them, and sends progress updates back.
fn engine_worker(
    cmd_rx: std_mpsc::Receiver<EngineCommand>,
    progress_tx: std_mpsc::Sender<ProgressUpdate>,
) {
    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            EngineCommand::Cancel => {
                // handled by AtomicBool
            }
            EngineCommand::Pause | EngineCommand::Resume => {
                // handled by AtomicBool
            }
            EngineCommand::Start {
                src,
                dst,
                recursive,
                verify,
                hash_algo,
                threads,
            } => {
                let log = vec![format!("Scanning: {}", src.display())];
                let _ = progress_tx.send(ProgressUpdate {
                    file: String::new(),
                    bytes_copied: 0,
                    total_bytes: 0,
                    speed: 0.0,
                    is_done: false,
                    error: String::new(),
                    log,
                });

                let files = match engine::collect_files(&src, &dst, recursive) {
                    Ok(f) => f,
                    Err(e) => {
                        let _ = progress_tx.send(ProgressUpdate {
                            file: String::new(),
                            bytes_copied: 0,
                            total_bytes: 0,
                            speed: 0.0,
                            is_done: true,
                            error: e.to_string(),
                            log: vec![format!("Error: {}", e)],
                        });
                        continue;
                    }
                };

                if files.is_empty() {
                    let _ = progress_tx.send(ProgressUpdate {
                        file: String::new(),
                        bytes_copied: 0,
                        total_bytes: 0,
                        speed: 0.0,
                        is_done: true,
                        error: "No files to copy".to_string(),
                        log: vec!["No files to copy".to_string()],
                    });
                    continue;
                }

                let log = vec![format!("Found {} file(s).", files.len())];
                let _ = progress_tx.send(ProgressUpdate {
                    file: String::new(),
                    bytes_copied: 0,
                    total_bytes: 0,
                    speed: 0.0,
                    is_done: false,
                    error: String::new(),
                    log,
                });

                let config = Arc::new(CopyConfig {
                    source: src,
                    destination: dst,
                    verify,
                    hash_algorithm: hash_algo,
                    threads,
                    recursive,
                    overwrite: OverwriteMode::Always,
                });

                // Bridge the unbounded channel used by the engine to a
                // bounded std::mpsc that the GUI polls.
                let (engine_tx, mut engine_rx) = tokio::sync::mpsc::unbounded_channel::<FileProgress>();
                let progress_for_bridge = progress_tx.clone();
                let bridge = std::thread::spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .unwrap();
                    rt.block_on(async {
                        while let Some(p) = engine_rx.recv().await {
                            let _ = progress_for_bridge.send(ProgressUpdate {
                                file: p.file.display().to_string(),
                                bytes_copied: p.bytes_copied,
                                total_bytes: p.total_bytes,
                                speed: p.speed_bytes_per_sec,
                                is_done: false,
                                error: String::new(),
                                log: vec![],
                            });
                        }
                    });
                });

                let cancel = Arc::new(AtomicBool::new(false));
                let pause = Arc::new(AtomicBool::new(false));
                let bytes_total = Arc::new(AtomicU64::new(0));
                let bytes_done = Arc::new(AtomicU64::new(0));

                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();

                let result = rt.block_on(async {
                    engine::run_copy_engine(files, config, engine_tx, cancel, Some(pause), bytes_total, bytes_done).await
                });

                drop(bridge);

                let is_done = true;
                let error = result.as_ref().err().map(|e| format!("{:#}", e)).unwrap_or_default();
                let log = match &result {
                    Ok(_) => vec!["✓ Copy complete!".to_string()],
                    Err(e) => vec![format!("✗ Error: {}", e)],
                };
                let _ = progress_tx.send(ProgressUpdate {
                    file: String::new(),
                    bytes_copied: 0,
                    total_bytes: 0,
                    speed: 0.0,
                    is_done,
                    error,
                    log,
                });
            }
        }
    }
}

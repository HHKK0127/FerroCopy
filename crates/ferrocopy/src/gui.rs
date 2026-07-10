//! FerroCopy GUI — egui-based interface for the file copy engine
//!
//! ZED-inspired features:
//!   - StatusBar: bottom bar with speed/elapsed/ETA/file count (cf. ZED status_bar.rs)
//!   - Toast notifications: non-intrusive auto-dismiss popups (cf. ZED toast_layer.rs)
//!   - Pause/Resume: task lifecycle management (cf. ZED tasks.rs)

use crate::config::*;
use crate::dot;
use crate::engine;
use eframe::egui;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::{Arc, Mutex};
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

/// Launch GUI with pre-populated source files (shell copy)
pub fn run_gui_with_sources(sources: Vec<String>) {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 650.0])
            .with_resizable(true)
            .with_title("FerroCopy — Shell Copy"),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "FerroCopy",
        options,
        Box::new(|_cc| Ok(Box::new(FerroCopyApp::with_sources(sources)) as Box<dyn eframe::App>)),
    );
}

/// Launch GUI with source files in move mode (shell move, deletes after copy)
pub fn run_gui_with_sources_move(sources: Vec<String>) {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 650.0])
            .with_resizable(true)
            .with_title("FerroCopy — Shell Move"),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "FerroCopy",
        options,
        Box::new(|_cc| {
            Ok(Box::new(FerroCopyApp::with_sources_move(sources)) as Box<dyn eframe::App>)
        }),
    );
}

/// Launch GUI with a pre-set destination (shell paste from folder bg)
pub fn run_gui_with_destination(dest: String) {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 650.0])
            .with_resizable(true)
            .with_title("FerroCopy — Paste"),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "FerroCopy",
        options,
        Box::new(|_cc| Ok(Box::new(FerroCopyApp::with_destination(dest)) as Box<dyn eframe::App>)),
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
    #[allow(dead_code)]
    selected: bool,
    #[allow(dead_code)]
    error: String,
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
    #[allow(dead_code)]
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
    // Shell integration: pre-populated sources from context menu
    shell_sources: Vec<String>,
    // Shell integration: move mode (delete after copy)
    move_mode: bool,
    // Shell integration: flag to process pending sources
    pending_add: bool,
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
            shell_sources: Vec::new(),
            move_mode: false,
            pending_add: false,
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
    /// Dot design: background particle system
    particle_system: dot::ParticleSystem,
    /// Animation time accumulator
    time: f64,
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
            particle_system: dot::ParticleSystem::new(60),
            time: 0.0,
        }
    }

    /// Create app with pre-populated source files (shell context menu)
    pub fn with_sources(sources: Vec<String>) -> Self {
        let app = Self::new();
        {
            let mut state = app.state.lock().unwrap();
            state.shell_sources = sources;
            state.pending_add = true;
        }
        app
    }

    /// Create app in move mode (delete after copy) with sources
    pub fn with_sources_move(sources: Vec<String>) -> Self {
        let app = Self::new();
        {
            let mut state = app.state.lock().unwrap();
            state.shell_sources = sources;
            state.move_mode = true;
            state.pending_add = true;
        }
        app
    }

    /// Create app with a pre-set destination (shell paste)
    pub fn with_destination(dest: String) -> Self {
        let app = Self::new();
        {
            let mut state = app.state.lock().unwrap();
            state.destination = dest;
        }
        app
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

        // ── Dot design: accumulate animation time ──
        let dt = ctx.input(|i| i.unstable_dt) as f32;
        self.time += dt as f64;

        // ── Dot design: background particle layer ──
        let ps = &mut self.particle_system;
        egui::Area::new("particle_bg".into())
            .order(egui::Order::Background)
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                ps.update(dt, &rect);
                let painter = ui.painter();
                ps.paint(painter, &rect);
            });

        // ── Top bar ──
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.heading("⚡ FerroCopy");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if dot::dot_button(ui, "About", true, self.time) {
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
            dot::dot_separator(ui, self.time);

            // ── Shell integration: process pending sources from context menu ──
            if state.pending_add && !state.shell_sources.is_empty() {
                let sources = state.shell_sources.clone();
                state.shell_sources.clear();
                state.pending_add = false;
                if sources.len() == 1 {
                    state.source = sources[0].clone();
                } else {
                    if let Some(parent) = std::path::Path::new(&sources[0]).parent() {
                        state.source = parent.display().to_string();
                    } else {
                        state.source = sources[0].clone();
                    }
                }
                for path in &sources {
                    if !state.files.iter().any(|f| f.name == *path) {
                        let path_obj = std::path::Path::new(path);
                        let size = crate::engine::get_file_size(path_obj);
                        state.files.push(FileEntry {
                            name: path.clone(),
                            size,
                            progress: 0.0,
                            status: "Pending".to_string(),
                            selected: true,
                            error: String::new(),
                        });
                    }
                }
                state
                    .log
                    .push(format!("📥 Shell: {} file(s) loaded", sources.len()));
                if state.move_mode {
                    state
                        .log
                        .push("🔁 Move mode: files will be deleted after copy".to_string());
                }
            }

            // ── Shell integration: move mode indicator ──
            if state.move_mode {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::GOLD, "🔁 Move Mode");
                    ui.label("(Source files will be deleted after copy)");
                });
            }

            // ── Source ──
            ui.horizontal(|ui| {
                ui.label("Source:");
                let mut src = state.source.clone();
                if ui
                    .add(egui::TextEdit::singleline(&mut src).desired_width(400.0))
                    .changed()
                {
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
                if ui
                    .add(egui::TextEdit::singleline(&mut dst).desired_width(400.0))
                    .changed()
                {
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

            dot::dot_separator(ui, self.time);

            // ── Options ──
            ui.horizontal(|ui| {
                dot::dot_checkbox(ui, &mut state.recursive, self.time);
                ui.label("Recursive");
                dot::dot_checkbox(ui, &mut state.verify, self.time);
                ui.label("Verify hash");
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

            dot::dot_separator(ui, self.time);

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
                start_clicked = dot::dot_button(ui, "▶ Start", can_start, self.time);
                pause_clicked = dot::dot_button(ui, "⏸ Pause", is_copying, self.time);
                resume_clicked = dot::dot_button(ui, "▶ Resume", is_paused, self.time);
                cancel_clicked = dot::dot_button(ui, "⏹ Cancel", is_copying || is_paused, self.time);
                let show_clear = matches!(state.status, AppStatus::Done | AppStatus::Error);
                clear_clicked = show_clear && dot::dot_button(ui, "Clear", show_clear, self.time);
            });

            // Snapshot values to use after dropping the lock
            let start_args: Option<(PathBuf, PathBuf, bool, bool, HashAlgorithm, usize)> =
                if start_clicked {
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
                    src,
                    dst,
                    recursive,
                    verify,
                    hash_algo,
                    threads,
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
                                dot::dot_progress_bar(ui, entry.progress, 120.0, false, self.time);
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
            if matches!(
                state.status,
                AppStatus::Copying | AppStatus::Paused | AppStatus::Done
            ) {
                dot::dot_separator(ui, self.time);
                let total = self.bytes_total.load(Ordering::Relaxed);
                let done = self.bytes_done.load(Ordering::Relaxed);
                if total > 0 {
                    ui.label(format!(
                        "{} / {}",
                        humansize::format_size(done, humansize::BINARY),
                        humansize::format_size(total, humansize::BINARY),
                    ));
                    let ratio = done as f32 / total as f32;
                    let paused = state.status == AppStatus::Paused;
                    dot::dot_progress_bar(ui, ratio, ui.available_width(), paused, self.time);
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
                dot::dot_separator(ui, self.time);
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
                let state_lock = self.state.lock().unwrap();
                let status = state_lock.status.clone();
                let file = state_lock.current_file.clone();
                let elapsed = state_lock.elapsed_secs;
                let eta = state_lock.eta_secs;
                let speed = state_lock.speed;
                drop(state_lock);
                let done = self.bytes_done.load(Ordering::Relaxed);
                let total = self.bytes_total.load(Ordering::Relaxed);

                // Left group: status + current file
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
                    AppStatus::Done => {
                        format!("{} copied", humansize::format_size(done, humansize::BINARY))
                    }
                    AppStatus::Error => "Error".into(),
                };
                ui.label(&status_str);

                // Right group (pushed to the right)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if elapsed > 0.0 {
                        ui.label(format!("⏱ {:.0}s", elapsed));
                    }
                    if eta > 0.0 && eta.is_finite() {
                        ui.label(format!("⏳ {:.0}s", eta));
                    }
                    if total > 0 {
                        let pct = done as f64 / total as f64 * 100.0;
                        ui.label(format!("📊 {:.0}%", pct));
                    }
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
                    .stroke(egui::Stroke::new(1.0_f32, color));
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

                // Single tokio runtime for both bridge and engine
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();

                let (engine_tx, engine_rx) =
                    tokio::sync::mpsc::unbounded_channel::<FileProgress>();
                let progress_for_bridge = progress_tx.clone();

                let cancel = Arc::new(AtomicBool::new(false));
                let pause = Arc::new(AtomicBool::new(false));
                let bytes_total = Arc::new(AtomicU64::new(0));
                let bytes_done = Arc::new(AtomicU64::new(0));

                let result = rt.block_on(async {
                    // Spawn bridge as a local task within the same runtime
                    let bridge_handle = tokio::spawn({
                        let mut rx = engine_rx;
                        let tx = progress_for_bridge;
                        async move {
                            while let Some(p) = rx.recv().await {
                                let _ = tx.send(ProgressUpdate {
                                    file: p.file.display().to_string(),
                                    bytes_copied: p.bytes_copied,
                                    total_bytes: p.total_bytes,
                                    speed: p.speed_bytes_per_sec,
                                    is_done: false,
                                    error: String::new(),
                                    log: vec![],
                                });
                            }
                        }
                    });

                    // Engine awaits semaphore; use the bridge handle's runtime
                    let engine_result = engine::run_copy_engine(
                        files,
                        config,
                        engine_tx,
                        cancel,
                        Some(pause),
                        bytes_total,
                        bytes_done,
                    )
                    .await;

                    let _ = bridge_handle.await;

                    engine_result
                });

                let (is_done, error, log) = {
                    let eng = result;
                    let err_count = eng.errors().count();
                    let err_msg = if err_count > 0 {
                        let first = eng.errors().next().map(|o| o.message.clone()).unwrap_or_default();
                        format!("{} error(s): {}", err_count, first)
                    } else {
                        String::new()
                    };
                    let log = if err_count > 0 {
                        let mut lines = vec![format!("✗ {} file(s) had errors", err_count)];
                        for o in eng.errors() {
                            lines.push(format!("  ✗ {}: {}", o.src.display(), o.message));
                        }
                        lines
                    } else {
                        let n = eng.outcomes.len();
                        vec![format!("✓ Copy complete: {} file(s) OK", n)]
                    };
                    (true, err_msg, log)
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

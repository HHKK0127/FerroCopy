//! CrashReporter — lightweight panic capture and crash dump writer.
//!
//! Hooks into `std::panic::set_hook` to capture panics, write a
//! crash dump to disk (with timestamp, version, and backtrace),
//! and show a user-facing message box on Windows.
//!
//! Lightweight alternative to Sentry for local use.
//! Upgrade path: replace hooks with `sentry` crate integration.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Global flag: panic hook is installed at most once.
static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Crash dump output directory (lazily resolved).
static CRASH_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Resolve crash directory: next to the executable.
fn crash_dir() -> &'static PathBuf {
    CRASH_DIR.get_or_init(|| {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("crash_reports")
    })
}

#[cfg(target_os = "windows")]
mod win_msgbox {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONERROR, MB_OK, MESSAGEBOX_STYLE,
    };

    /// Show a Windows message box with the given text and title.
    pub fn show(text: &str, title: &str) {
        let text_wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let title_wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            MessageBoxW(
                HWND(std::ptr::null_mut()),
                PCWSTR::from_raw(text_wide.as_ptr()),
                PCWSTR::from_raw(title_wide.as_ptr()),
                MESSAGEBOX_STYLE(MB_OK.0 | MB_ICONERROR.0),
            );
        }
    }
}

/// Install a panic hook that writes crash dumps to disk.
///
/// Writes to `crash_reports/ferrocopy_crash_<timestamp>.txt`.
/// On Windows, also shows a message box.
pub fn install_crash_handler() {
    use std::panic;

    if HOOK_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }

    let prev = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let timestamp_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let panic_msg = panic_info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| panic_info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "Unknown panic".to_string());

        let location = panic_info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown".to_string());

        let backtrace = std::backtrace::Backtrace::force_capture();

        let report = format!(
            "FerroCopy Crash Report\n\
             =====================\n\
             Version: {}\n\
             Timestamp (unix): {}\n\
             Panic: {}\n\
             Location: {}\n\
             \n\
             Backtrace:\n\
             {}\n",
            env!("CARGO_PKG_VERSION"),
            timestamp_secs,
            panic_msg,
            location,
            backtrace,
        );

        // Write crash dump
        let dir = crash_dir();
        let _ = std::fs::create_dir_all(dir);
        let filename = format!("ferrocopy_crash_{}.txt", timestamp_secs);
        let crash_path = dir.join(&filename);

        if let Err(e) = std::fs::write(&crash_path, &report) {
            eprintln!("Failed to write crash report: {}", e);
        } else {
            eprintln!("Crash report written to: {}", crash_path.display());
        }

        // Windows: show error dialog
        #[cfg(target_os = "windows")]
        {
            let msg = format!(
                "FerroCopy がクラッシュしました\n\n\
                 エラー: {}\n\
                 ロケーション: {}\n\n\
                 クラッシュレポート:\n{}",
                panic_msg, location, crash_path.display()
            );
            win_msgbox::show(&msg, "FerroCopy Crash");
        }

        prev(panic_info);
    }));
}
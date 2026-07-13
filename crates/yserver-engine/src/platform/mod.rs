//! Platform abstraction layer for yserver-engine.
//!
//! Provides OS-specific surface creation and clipboard access.
//! Currently supports Windows (Win32); stubs for other platforms.

cfg_if::cfg_if! {
    if #[cfg(target_os = "windows")] {
        mod windows;
        pub use windows::*;
    } else if #[cfg(target_os = "linux")] {
        mod linux;
        pub use linux::*;
    } else if #[cfg(target_os = "macos")] {
        mod macos;
        pub use macos::*;
    }
}

/// Platform capabilities reported at runtime.
#[derive(Debug, Clone, Default)]
pub struct PlatformInfo {
    /// OS name (e.g. "windows", "linux", "macos")
    pub os: &'static str,
    /// Whether the compositor surface is natively supported
    pub has_native_surface: bool,
    /// Whether clipboard integration is available
    pub has_clipboard: bool,
    /// DPI scale factor
    pub dpi_scale: f64,
}

/// Get platform information.
pub fn platform_info() -> PlatformInfo {
    #[cfg(target_os = "windows")]
    return windows::platform_info_impl();
    #[cfg(target_os = "linux")]
    return linux::platform_info_impl();
    #[cfg(target_os = "macos")]
    return macos::platform_info_impl();
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    PlatformInfo::default()
}

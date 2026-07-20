use super::PlatformInfo;

pub fn platform_info_impl() -> PlatformInfo {
    PlatformInfo {
        os: "macos",
        has_native_surface: true,
        has_clipboard: false, // macOS clipboard requires special entitlements
        dpi_scale: 1.0,
    }
}
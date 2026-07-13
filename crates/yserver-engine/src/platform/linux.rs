use super::PlatformInfo;

pub fn platform_info_impl() -> PlatformInfo {
    PlatformInfo {
        os: "linux",
        has_native_surface: true,
        has_clipboard: true,
        dpi_scale: 1.0,
    }
}
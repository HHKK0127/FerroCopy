//! Build script that ensures the test WASM plugin is up-to-date.
//! Runs `cargo build --release --target wasm32-wasip1` in the test-plugin dir.

fn main() {
    // Only build when running tests with --release
    if std::env::var("PROFILE").map_or(false, |p| p == "release") {
        let plugin_dir = std::path::Path::new("tests/plugins/test-plugin");
        if plugin_dir.exists() {
            let status = std::process::Command::new("cargo")
                .args(["build", "--release", "--target", "wasm32-wasip1"])
                .current_dir(plugin_dir)
                .status()
                .expect("Failed to build test WASM plugin");
            if !status.success() {
                panic!("Test WASM plugin build failed");
            }
            // Copy to expected location
            let src = plugin_dir.join("target/wasm32-wasip1/release/ferrocopy_test_plugin.wasm");
            let dst = std::path::Path::new("tests/plugins/test_plugin.wasm");
            if src.exists() {
                std::fs::copy(&src, &dst).expect("Failed to copy WASM plugin");
            }
        }
    }
}
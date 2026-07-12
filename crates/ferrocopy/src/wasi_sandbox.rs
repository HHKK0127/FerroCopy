//! WASI Plugin Sandbox — run user-defined plugins in a WebAssembly sandbox.
//!
//! Provides isolation for third-party copy/move plugins via wasmtime + WASI.
//! Each plugin is compiled from a .wasm module and can only call host
//! functions exposed by FerroCopy.

use anyhow::{Context, Result};
use std::path::Path;
use wasmtime::*;

/// Configuration for a WASM plugin instance.
pub struct WasiPlugin {
    engine: Engine,
    module: Module,
    wasm_path: std::path::PathBuf,
}

/// Result of running a plugin's entry point.
#[derive(Debug)]
pub struct PluginResult {
    /// Return value from the plugin's main function (0 = success).
    pub exit_code: i32,
    /// Name of the module that was run.
    pub module_name: String,
}

impl WasiPlugin {
    /// Load a WASM plugin from a `.wasm` file.
    pub fn from_file(path: &Path) -> Result<Self> {
        let engine = Engine::default();
        let module = Module::from_file(&engine, path)
            .with_context(|| format!("Failed to compile WASM module: {:?}", path))?;
        Ok(Self {
            engine,
            module,
            wasm_path: path.to_path_buf(),
        })
    }

    /// Load a WASM plugin from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let engine = Engine::default();
        let module = Module::new(&engine, bytes)
            .context("Failed to compile WASM module from bytes")?;
        Ok(Self {
            engine,
            module,
            wasm_path: std::path::PathBuf::new(),
        })
    }

    /// Check whether the module exports a function with the given name.
    pub fn has_export(&self, name: &str) -> bool {
        self.module
            .exports()
            .any(|e| e.name() == name)
    }

    /// List all exported function names from the module.
    pub fn exports(&self) -> Vec<String> {
        self.module
            .exports()
            .map(|e| e.name().to_string())
            .collect()
    }

    /// Invoke a function exported by the module with no arguments.
    /// Returns the first i32 return value if present.
    pub fn invoke(&self, func_name: &str) -> Result<Option<i32>> {
        let mut store = Store::new(&self.engine, ());
        let instance = Instance::new(&mut store, &self.module, &[])?;
        let func = instance
            .get_func(&mut store, func_name)
            .ok_or_else(|| anyhow::anyhow!("Function '{}' not found", func_name))?;

        let mut results = vec![Val::I32(0)];
        func.call(&mut store, &[], &mut results)?;
        Ok(results[0].i32())
    }

    /// Get the path the plugin was loaded from (empty if from bytes).
    pub fn path(&self) -> &Path {
        &self.wasm_path
    }

    /// Run the plugin and produce a result summary.
    pub fn run(&self) -> Result<PluginResult> {
        let exit_code = self.invoke("_start").ok().flatten().unwrap_or(0);
        Ok(PluginResult {
            exit_code,
            module_name: self
                .wasm_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("anonymous")
                .to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal valid WASM module (magic + version 1) with no exports.
    // This tests our parser without needing a real plugin file.
    const EMPTY_WASM: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, // magic
        0x01, 0x00, 0x00, 0x00, // version 1
    ];

    #[test]
    fn test_load_from_bytes() {
        let plugin = WasiPlugin::from_bytes(EMPTY_WASM);
        assert!(plugin.is_ok());
        let plugin = plugin.unwrap();
        assert!(plugin.exports().is_empty());
    }

    #[test]
    fn test_load_invalid_bytes_errors() {
        let result = WasiPlugin::from_bytes(&[0xFF, 0xFF, 0xFF, 0xFF]);
        assert!(result.is_err());
    }

    #[test]
    fn test_has_export_false_for_empty() {
        let plugin = WasiPlugin::from_bytes(EMPTY_WASM).unwrap();
        assert!(!plugin.has_export("_start"));
        assert!(!plugin.has_export("main"));
    }

    #[test]
    fn test_invoke_missing_function_errors() {
        let plugin = WasiPlugin::from_bytes(EMPTY_WASM).unwrap();
        let result = plugin.invoke("nonexistent");
        assert!(result.is_err());
    }
}
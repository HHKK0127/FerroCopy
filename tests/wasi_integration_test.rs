//! Integration test for the test WASM plugin.
//! Validates that test_plugin.wasm loads and exports the expected functions.
//! Uses wasmtime directly (the same engine wasi_sandbox.rs uses).

use wasmtime::*;

const TEST_PLUGIN_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/plugins/test_plugin.wasm"
);

fn load_module() -> (Engine, Module) {
    let path = std::path::Path::new(TEST_PLUGIN_PATH);
    assert!(path.exists(), "Test WASM plugin not found: {}", path.display());
    let engine = Engine::default();
    let module = Module::from_file(&engine, path)
        .expect("Failed to compile WASM module");
    (engine, module)
}

#[test]
fn test_wasm_plugin_loads() {
    let (_engine, module) = load_module();
    let exports: Vec<String> = module.exports().map(|e| e.name().to_string()).collect();
    assert!(exports.contains(&"_start".to_string()), "Expected _start export, got: {:?}", exports);
    assert!(exports.contains(&"filter_file".to_string()), "Expected filter_file export, got: {:?}", exports);
}

#[test]
fn test_wasm_plugin_invoke_filter_file() {
    let (engine, module) = load_module();
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("Failed to instantiate");
    let func = instance.get_func(&mut store, "filter_file").expect("filter_file not found");
    let mut results = vec![Val::I32(0)];
    func.call(&mut store, &[], &mut results).expect("invoke failed");
    assert_eq!(results[0].i32(), Some(1), "filter_file should return 1 (accept)");
}

#[test]
fn test_wasm_plugin_invoke_start() {
    let (engine, module) = load_module();
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("Failed to instantiate");
    let func = instance.get_func(&mut store, "_start").expect("_start not found");
    let mut results = vec![Val::I32(0)];
    func.call(&mut store, &[], &mut results).expect("invoke failed");
    assert_eq!(results[0].i32(), Some(0), "_start should return 0 (success)");
}

#[test]
fn test_wasm_plugin_load_from_bytes() {
    let wasm_bytes = std::fs::read(TEST_PLUGIN_PATH).expect("Failed to read test WASM file");
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes).expect("Failed to compile from bytes");
    let exports: Vec<String> = module.exports().map(|e| e.name().to_string()).collect();
    assert!(exports.contains(&"filter_file".to_string()));
}
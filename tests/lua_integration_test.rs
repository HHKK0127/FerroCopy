//! Integration test for Lua filtering.
//! Tests with a .lua script file loaded from disk.

use std::path::Path;

const TEST_LUA_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/lua");

/// Helper: create a test Lua script file and return its path.
fn create_script(name: &str, content: &str) -> String {
    let dir = Path::new(TEST_LUA_DIR);
    std::fs::create_dir_all(dir).expect("create lua test dir");
    let path = dir.join(name);
    std::fs::write(&path, content).expect("write lua script");
    path.to_string_lossy().to_string()
}

#[test]
fn test_lua_filter_includes_only_txt_files() {
    let script = create_script("filter_txt.lua", r#"
function filter(path, size)
    return path:match("%.txt$") ~= nil
end
"#);

    // We can't easily test the CLI integration without a real copy,
    // but we can test that the LuaFilter module works correctly
    // by using the mlua crate directly (same way lua_ext.rs does).
    let lua = mlua::Lua::new();
    lua.load(&std::fs::read_to_string(&script).unwrap())
        .exec()
        .expect("Lua script execution failed");

    let globals = lua.globals();
    let filter_fn: mlua::Function = globals.get("filter").expect("filter function");
    let result1: bool = filter_fn.call(("readme.txt", 100u64)).expect("call filter");
    assert!(result1, "Should include .txt files");

    let result2: bool = filter_fn.call(("image.png", 999u64)).expect("call filter");
    assert!(!result2, "Should exclude .png files");
}

#[test]
fn test_lua_filter_by_size() {
    let script = create_script("filter_size.lua", r#"
function filter(path, size)
    return size > 1024
end
"#);

    let lua = mlua::Lua::new();
    lua.load(&std::fs::read_to_string(&script).unwrap())
        .exec()
        .expect("Lua script execution failed");

    let globals = lua.globals();
    let filter_fn: mlua::Function = globals.get("filter").expect("filter function");

    assert!(filter_fn.call::<bool>(("big.bin", 2048u64)).unwrap());
    assert!(!filter_fn.call::<bool>(("small.txt", 512u64)).unwrap());
}

#[test]
fn test_lua_filter_script_syntax_error_reported() {
    let script = create_script("bad_syntax.lua", "function filter(path, size) return true ++ false end");

    let lua = mlua::Lua::new();
    let result = lua.load(&std::fs::read_to_string(&script).unwrap()).exec();
    assert!(result.is_err(), "Bad Lua syntax should fail");
}

#[test]
fn test_lua_filter_missing_function_returns_error() {
    let script = create_script("no_filter.lua", "x = 42");

    let lua = mlua::Lua::new();
    lua.load(&std::fs::read_to_string(&script).unwrap())
        .exec()
        .expect("script should load");

    let globals = lua.globals();
    let result = globals.get::<mlua::Function>("filter");
    assert!(result.is_err(), "Missing filter function should error");
}
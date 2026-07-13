//! LuaExt — Lua-based filtering and scripting (WezTerm-inspired).
//!
//! Provides a lightweight Lua runtime for user-defined filtering rules
//! (e.g., skip certain file patterns) and simple automation scripts.

use mlua::{Function, Lua, Result as LuaResult, Value};

/// A reusable Lua scripting context for filtering.
pub struct LuaFilter {
    lua: Lua,
    script: String,
}

impl LuaFilter {
    /// Create a new Lua filter from script source.
    ///
    /// The script should define a global `filter(path: string, size: int) -> bool`
    /// function that returns `true` to include the file or `false` to skip it.
    pub fn new(script: &str) -> Result<Self, String> {
        let lua = Lua::new();
        lua.load(script)
            .exec()
            .map_err(|e| format!("Lua script error: {}", e))?;
        Ok(Self {
            lua,
            script: script.to_string(),
        })
    }

    /// Run the filter function on a file path.
    /// Returns `true` = include, `false` = skip.
    pub fn should_include(&self, path: &str, size: u64) -> Result<bool, String> {
        let globals = self.lua.globals();
        let filter_fn: Function = globals
            .get("filter")
            .map_err(|e| format!("Missing 'filter' function: {}", e))?;

        match filter_fn.call::<bool>((path, size)) {
            Ok(result) => Ok(result),
            Err(e) => Err(format!("Lua filter error: {}", e)),
        }
    }

    /// Get the original script text.
    pub fn script(&self) -> &str {
        &self.script
    }

    /// Return a list of custom globals defined in the script (excluding Lua builtins).
    pub fn custom_globals(&self) -> Vec<String> {
        let globals = self.lua.globals();
        let mut keys: Vec<String> = Vec::new();
        for pair in globals.pairs::<String, Value>() {
            if let Ok((key, _)) = pair {
                if !key.starts_with('_') && key != "filter" {
                    keys.push(key);
                }
            }
        }
        keys
    }
}

/// Run an arbitrary Lua expression and return its string representation.
pub fn eval(script: &str) -> LuaResult<String> {
    let lua = Lua::new();
    let result: Value = lua.load(script).eval()?;
    Ok(match result {
        Value::Nil => "nil".into(),
        Value::Boolean(b) => b.to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.to_string_lossy().to_string(),
        Value::Table(t) => format!("table: {} entries", t.len().unwrap_or(0)),
        _ => format!("{:?}", result),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_include_large_files() {
        let script = r#"
function filter(path, size)
    return size > 1024
end
"#;
        let filter = LuaFilter::new(script).unwrap();
        assert!(filter.should_include("big.bin", 2048).unwrap());
        assert!(!filter.should_include("small.txt", 512).unwrap());
    }

    #[test]
    fn test_filter_by_extension() {
        let script = r#"
function filter(path, size)
    return path:match("%.txt$") ~= nil
end
"#;
        let filter = LuaFilter::new(script).unwrap();
        assert!(filter.should_include("readme.txt", 100).unwrap());
        assert!(!filter.should_include("image.png", 99999).unwrap());
    }

    #[test]
    fn test_invalid_script_errors() {
        let result = LuaFilter::new("invalid lua syntax {{");
        assert!(result.is_err());
    }

    #[test]
    fn test_eval_expression() {
        assert_eq!(eval("2 + 2").unwrap(), "4");
        assert_eq!(eval("\"hello\" .. \" world\"").unwrap(), "hello world");
    }

    #[test]
        fn test_custom_globals() {
        let script = r#"
    custom_value = 42
    function filter(path, size)
        return true
    end
    "#;
            let filter = LuaFilter::new(script).unwrap();
            let globals = filter.custom_globals();
            assert!(globals.contains(&"custom_value".to_string()));
        }
}

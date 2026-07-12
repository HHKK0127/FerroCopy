//! Plugin — pluggable copy command architecture.
//!
//! Inspired by Bevy's Plugin trait and Lapce's command dispatcher.
//! Allows external code to register custom hooks around copy operations.

use crate::config::CopyConfig;
use std::path::PathBuf;

/// Context passed to a plugin when a copy command is dispatched.
#[derive(Debug, Clone)]
pub struct PluginContext {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub config: CopyConfig,
}

/// Commands that can be dispatched to plugins.
#[derive(Debug, Clone)]
pub enum CopyCommand {
    /// Called before any files are copied.
    PreCopy(PluginContext),
    /// Called after a single file is copied.
    PostFile {
        src: PathBuf,
        dst: PathBuf,
        bytes: u64,
        success: bool,
    },
    /// Called after all files are copied.
    PostCopy {
        total_files: u64,
        total_bytes: u64,
        errors: u64,
    },
}

/// A plugin that can hook into copy operations.
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;

    /// Called when a command is dispatched. Return Ok to continue,
    /// Err to abort the operation.
    fn handle(&self, _cmd: &CopyCommand) -> Result<(), String> {
        Ok(())
    }
}

/// Registry that holds all plugins and dispatches commands.
pub struct PluginRegistry {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self { plugins: Vec::new() }
    }

    /// Register a plugin.
    pub fn register<P: Plugin + 'static>(&mut self, plugin: P) {
        tracing::info!("Registered plugin: {}", plugin.name());
        self.plugins.push(Box::new(plugin));
    }

    /// Dispatch a command to all plugins in registration order.
    /// Returns the first error encountered, if any.
    pub fn dispatch(&self, cmd: &CopyCommand) -> Result<(), String> {
        for plugin in &self.plugins {
            plugin.handle(cmd)?;
        }
        Ok(())
    }

    /// Number of registered plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// A simple logging plugin for testing.
pub struct LogPlugin;

impl Plugin for LogPlugin {
    fn name(&self) -> &str {
        "log"
    }

    fn handle(&self, cmd: &CopyCommand) -> Result<(), String> {
        match cmd {
            CopyCommand::PreCopy(ctx) => {
                tracing::info!("PreCopy: {} → {}", ctx.source.display(), ctx.destination.display());
            }
            CopyCommand::PostFile { src, success, .. } => {
                if *success {
                    tracing::debug!("PostFile: {}", src.display());
                } else {
                    tracing::warn!("PostFile FAILED: {}", src.display());
                }
            }
            CopyCommand::PostCopy { total_files, errors, .. } => {
                tracing::info!("PostCopy: {} files, {} errors", total_files, errors);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    struct FailPlugin;
    impl Plugin for FailPlugin {
        fn name(&self) -> &str {
            "fail"
        }
        fn handle(&self, _cmd: &CopyCommand) -> Result<(), String> {
            Err("intentional failure".into())
        }
    }

    #[test]
    fn test_registry_dispatch() {
        let mut reg = PluginRegistry::new();
        reg.register(LogPlugin);
        assert_eq!(reg.len(), 1);

        let ctx = PluginContext {
            source: Path::new("a").to_path_buf(),
            destination: Path::new("b").to_path_buf(),
            config: CopyConfig {
                source: Path::new("a").to_path_buf(),
                destination: Path::new("b").to_path_buf(),
                verify: false,
                hash_algorithm: crate::config::HashAlgorithm::Blake3,
                threads: 1,
                recursive: false,
                overwrite: crate::config::OverwriteMode::Always,
            },
        };
        let cmd = CopyCommand::PreCopy(ctx);
        assert!(reg.dispatch(&cmd).is_ok());
    }

    #[test]
    fn test_fail_plugin() {
        let mut reg = PluginRegistry::new();
        reg.register(FailPlugin);
        let cmd = CopyCommand::PostCopy {
            total_files: 0,
            total_bytes: 0,
            errors: 0,
        };
        assert!(reg.dispatch(&cmd).is_err());
    }

    #[test]
    fn test_empty_registry() {
        let reg = PluginRegistry::new();
        assert!(reg.is_empty());
        let cmd = CopyCommand::PostCopy {
            total_files: 0,
            total_bytes: 0,
            errors: 0,
        };
        assert!(reg.dispatch(&cmd).is_ok());
    }
}
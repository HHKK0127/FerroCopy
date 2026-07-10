// ── Bevy-inspired: Plugin architecture ───────────────────────────────
//
// Plugin trait + App builder pattern for modular feature composition.
// Each capability (GUI, CLI, Shell, Verification, SSH) becomes a Plugin
// that registers its own commands, UI elements, and event handlers.

use crate::config::FerroConfig;
use crate::engine::EngineResult;
use std::sync::Arc;

/// Unique identifier for each plugin.
/// Used for dependency ordering and registration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PluginId(pub &'static str);

impl PluginId {
    pub const CORE: Self = PluginId("ferrocopy.core");
    pub const GUI: Self = PluginId("ferrocopy.gui");
    pub const CLI: Self = PluginId("ferrocopy.cli");
    pub const SHELL: Self = PluginId("ferrocopy.shell");
    pub const VERIFY: Self = PluginId("ferrocopy.verify");
    pub const SSH: Self = PluginId("ferrocopy.ssh");
}

/// Plugin dependency: one plugin must be loaded before another.
#[derive(Debug, Clone)]
pub struct PluginDependency {
    pub id: PluginId,
    pub required: bool,
}

/// Bevy-inspired: every capability is a Plugin with lifecycle hooks.
pub trait Plugin: Send {
    /// Unique identifier for this plugin.
    fn id(&self) -> PluginId;

    /// List of plugins this plugin depends on.
    fn dependencies(&self) -> Vec<PluginDependency> {
        Vec::new()
    }

    /// Called once during app initialization.
    fn on_register(&self, app: &mut App) -> anyhow::Result<()>;

    /// Called after all plugins are registered.
    fn on_startup(&self, _app: &App) -> anyhow::Result<()> {
        Ok(())
    }

    /// Called periodically (or on config change).
    fn on_update(&self, _app: &App) -> anyhow::Result<()> {
        Ok(())
    }

    /// Called on graceful shutdown.
    fn on_shutdown(&self, _app: &App) -> anyhow::Result<()> {
        Ok(())
    }
}

/// The App registry: holds config and all registered plugins.
/// Bevy App inspired: single entry point for building FerroCopy.
pub struct App {
    pub config: FerroConfig,
    plugins: Vec<Box<dyn Plugin>>,
    plugin_ids: std::collections::HashSet<PluginId>,
}

impl App {
    pub fn new(config: FerroConfig) -> Self {
        Self {
            config,
            plugins: Vec::new(),
            plugin_ids: std::collections::HashSet::new(),
        }
    }

    /// Register a plugin. Returns error if already registered.
    pub fn add_plugin<T: Plugin + 'static>(&mut self, plugin: T) -> anyhow::Result<()> {
        let id = plugin.id();
        if !self.plugin_ids.insert(id.clone()) {
            anyhow::bail!("Plugin already registered: {:?}", id);
        }
        // Call register lifecycle
        plugin.on_register(self)?;
        self.plugins.push(Box::new(plugin));
        Ok(())
    }

    /// Call startup on all plugins (after all are registered).
    pub fn startup(&self) -> anyhow::Result<()> {
        for plugin in &self.plugins {
            plugin.on_startup(self)?;
        }
        Ok(())
    }

    /// Call update on all plugins.
    pub fn update(&self) -> anyhow::Result<()> {
        for plugin in &self.plugins {
            plugin.on_update(self)?;
        }
        Ok(())
    }

    /// Call shutdown on all plugins (reverse order).
    pub fn shutdown(&self) -> anyhow::Result<()> {
        for plugin in self.plugins.iter().rev() {
            plugin.on_shutdown(self)?;
        }
        Ok(())
    }
}

// ── Lapce-inspired: Command dispatcher ───────────────────────────────
//
// Centralized, type-safe command dispatch. CopyCommand variants
// map directly to user actions, removing ad-hoc match chains.

/// All copy-related commands, dispatched via a single channel.
/// Lapce dispatch.rs inspired: Start/Cancel/Pause as typed variants.
#[derive(Debug, Clone)]
pub enum CopyCommand {
    /// Start copy with given source(s) and destination.
    Start {
        sources: Vec<std::path::PathBuf>,
        destination: std::path::PathBuf,
    },
    /// Cancel current copy operation.
    Cancel,
    /// Pause current copy operation.
    Pause,
    /// Resume paused copy operation.
    Resume,
    /// Retry failed items.
    RetryFailed,
    /// Move mode (copy + delete source).
    Move {
        sources: Vec<std::path::PathBuf>,
        destination: std::path::PathBuf,
    },
}

/// Result of executing a CopyCommand.
#[derive(Debug)]
pub enum CommandResult {
    Started,
    Cancelled,
    Paused,
    Resumed,
    Completed(EngineResult),
    Error(String),
}

/// Trait for objects that can execute CopyCommands.
pub trait CommandHandler: Send {
    fn execute(&mut self, cmd: CopyCommand) -> CommandResult;
}

/// Default command dispatcher: routes commands to registered handlers.
pub struct Dispatcher {
    handlers: Vec<Box<dyn CommandHandler>>,
}

impl Dispatcher {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// Register a command handler (e.g., engine shell, GUI, CLI).
    pub fn add_handler(&mut self, handler: Box<dyn CommandHandler>) {
        self.handlers.push(handler);
    }

    /// Dispatch a command to all handlers (first match wins).
    pub fn dispatch(&mut self, cmd: CopyCommand) -> CommandResult {
        for handler in &mut self.handlers {
            let result = handler.execute(cmd.clone());
            match &result {
                CommandResult::Started
                | CommandResult::Cancelled
                | CommandResult::Paused
                | CommandResult::Resumed
                | CommandResult::Completed(_) => return result,
                CommandResult::Error(_) => continue,
            }
        }
        CommandResult::Error("No handler could process this command".into())
    }
}

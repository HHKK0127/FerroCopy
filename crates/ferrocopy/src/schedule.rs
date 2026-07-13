//! Schedule — Stage/System pipeline for copy operations.
//!
//! Inspired by Bevy's Schedule/Stages. Defines a dependency graph:
//!   Scan → Copy → Verify → Report
//! Each stage runs its systems in order, with stages executing sequentially.

use std::path::PathBuf;

/// A pipeline stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    Scan,
    Copy,
    Verify,
    Report,
}

impl Stage {
    /// Return stages in execution order.
    pub fn ordered() -> Vec<Stage> {
        vec![Stage::Scan, Stage::Copy, Stage::Verify, Stage::Report]
    }
}

/// Pipeline state shared between stages.
#[derive(Debug, Clone, Default)]
pub struct PipelineState {
    pub source: Option<PathBuf>,
    pub destination: Option<PathBuf>,
    pub files: Vec<(PathBuf, PathBuf)>,
    pub total_bytes: u64,
    pub bytes_copied: u64,
    pub verified: bool,
    pub errors: u64,
}

/// A system is a function that mutates the pipeline state.
pub trait System: Send {
    fn name(&self) -> &str;
    fn run(&self, state: &mut PipelineState) -> Result<(), String>;
}

/// The schedule runs systems grouped by stage.
pub struct Schedule {
    systems: Vec<(Stage, Box<dyn System>)>,
}

impl Schedule {
    pub fn new() -> Self {
        Self { systems: Vec::new() }
    }

    /// Add a system to a stage.
    pub fn add_system<S: System + 'static>(&mut self, stage: Stage, system: S) {
        self.systems.push((stage, Box::new(system)));
    }

    /// Run all systems in stage order.
    pub fn run(&self, state: &mut PipelineState) -> Result<(), String> {
        for stage in Stage::ordered() {
            tracing::info!("▶ Stage: {:?}", stage);
            for (s, system) in &self.systems {
                if *s == stage {
                    system.run(state)?;
                }
            }
        }
        Ok(())
    }
}

impl Default for Schedule {
    fn default() -> Self {
        Self::new()
    }
}

// ── Built-in systems ──────────────────────────────────────────────────

/// Scan system: collect files from source.
pub struct ScanSystem;

impl System for ScanSystem {
    fn name(&self) -> &str {
        "scan"
    }
    fn run(&self, state: &mut PipelineState) -> Result<(), String> {
        if let Some(src) = &state.source {
            if let Some(dst) = &state.destination {
                let files = crate::engine::collect_files(src, dst, true)
                    .map_err(|e| format!("Scan failed: {}", e))?;
                state.total_bytes = files
                    .iter()
                    .filter_map(|(s, _)| std::fs::metadata(s).ok().map(|m| m.len()))
                    .sum();
                state.files = files;
            }
        }
        Ok(())
    }
}

/// Report system: log final statistics.
pub struct ReportSystem;

impl System for ReportSystem {
    fn name(&self) -> &str {
        "report"
    }
    fn run(&self, state: &mut PipelineState) -> Result<(), String> {
        tracing::info!(
            "📋 Report: {} files, {} bytes, {} errors, verified={}",
            state.files.len(),
            state.bytes_copied,
            state.errors,
            state.verified
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stage_order() {
        let order = Stage::ordered();
        assert_eq!(order, vec![Stage::Scan, Stage::Copy, Stage::Verify, Stage::Report]);
    }

    #[test]
    fn test_report_system() {
        let mut state = PipelineState::default();
        state.files = vec![(PathBuf::from("a"), PathBuf::from("b"))];
        state.bytes_copied = 100;
        ReportSystem.run(&mut state).unwrap();
    }

    #[test]
    fn test_empty_schedule() {
        let schedule = Schedule::new();
        let mut state = PipelineState::default();
        assert!(schedule.run(&mut state).is_ok());
    }
}
//! FileWatcher — debounced directory watcher using notify-debouncer-full.
//!
//! Watches a directory recursively and reports file system changes
//! after a configurable debounce interval (Bevy-inspired).

use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

/// A debounced file system change event.
#[derive(Debug, Clone)]
pub enum FileChange {
    Created(PathBuf),
    Modified(PathBuf),
    Removed(PathBuf),
    Renamed(PathBuf, PathBuf),
    Any(PathBuf),
}

/// Watches a directory for file changes with configurable debouncing.
pub struct FileWatcher {
    /// Inner debouncer (wrapped in Option so we can take it in Drop).
    debouncer: Option<Debouncer<notify::RecommendedWatcher, FileIdMap>>,
    /// Channel receiving batches of changes.
    receiver: Receiver<Vec<FileChange>>,
    /// Background thread for event processing.
    thread: Option<thread::JoinHandle<()>>,
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        // Drop the debouncer first to stop the underlying watcher.
        // This closes the debounced channel, allowing the background
        // thread to exit naturally.
        drop(self.debouncer.take());
        // Now join the background thread safely.
        if let Some(h) = self.thread.take() {
            let _ = h.join();
        }
    }
}

impl FileWatcher {
    /// Start watching `path` with the given `debounce_ms` interval.
    pub fn watch(path: PathBuf, debounce_ms: u64) -> Result<Self, String> {
        let (tx, receiver) = mpsc::channel::<Vec<FileChange>>();

        let (debounced_tx, debounced_rx) = std::sync::mpsc::channel::<DebounceEventResult>();

        let mut debouncer = new_debouncer(
            Duration::from_millis(debounce_ms),
            None,
            move |result: DebounceEventResult| {
                let _ = debounced_tx.send(result);
            },
        )
        .map_err(|e| format!("Failed to create debouncer: {}", e))?;

        debouncer
            .watch(&path, RecursiveMode::Recursive)
            .map_err(|e| {
                // Debouncer drops cleanly when its return value is dropped.
                format!("Failed to watch path '{}': {}", path.display(), e)
            })?;

        let handle = thread::spawn(move || {
            while let Ok(result) = debounced_rx.recv() {
                if let Ok(events) = result {
                    let changes: Vec<FileChange> = events
                        .into_iter()
                        .filter_map(|de| {
                            let event = de.event;
                            let paths = event.paths.clone();
                            let first = paths.first()?.clone();
                            let kind = event.kind;
                            use notify::EventKind;
                            Some(match kind {
                                EventKind::Create(_) => FileChange::Created(first),
                                EventKind::Modify(notify::event::ModifyKind::Name(_)) => {
                                    let second = paths.get(1).cloned().unwrap_or_default();
                                    FileChange::Renamed(first, second)
                                }
                                EventKind::Modify(_) => FileChange::Modified(first),
                                EventKind::Remove(_) => FileChange::Removed(first),
                                _ => FileChange::Any(first),
                            })
                        })
                        .collect();
                    if !changes.is_empty() {
                        let _ = tx.send(changes);
                    }
                }
            }
        });

        Ok(Self {
            debouncer: Some(debouncer),
            receiver,
            thread: Some(handle),
        })
    }

    /// Non-blocking: return all pending file changes.
    pub fn poll(&self) -> Vec<FileChange> {
        self.receiver.try_recv().unwrap_or_default()
    }

    /// Blocking: wait for the next batch of changes.
    pub fn wait_for_changes(&self) -> Result<Vec<FileChange>, String> {
        self.receiver
            .recv()
            .map_err(|e| format!("Watcher channel closed: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_watch_detects_file_creation() {
        let dir = temp_dir("ferrocopy_watch_create");
        let watcher = FileWatcher::watch(dir.clone(), 50).unwrap();
        fs::write(dir.join("new.txt"), b"data").unwrap();
        thread::sleep(Duration::from_millis(400));
        let changes = watcher.poll();
        assert!(!changes.is_empty(), "Should detect file creation");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_watch_returns_empty_on_no_changes() {
        let dir = temp_dir("ferrocopy_watch_empty");
        let watcher = FileWatcher::watch(dir.clone(), 50).unwrap();
        thread::sleep(Duration::from_millis(100));
        assert!(watcher.poll().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_watch_invalid_path_errors() {
        let result = FileWatcher::watch(
            PathBuf::from(r"C:\__ferrocopy_nonexistent_path__"),
            100,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_watch_detects_modification() {
        let dir = temp_dir("ferrocopy_watch_modify");
        let file = dir.join("mod.txt");
        fs::write(&file, b"v1").unwrap();
        thread::sleep(Duration::from_millis(100));

        let watcher = FileWatcher::watch(dir.clone(), 50).unwrap();
        fs::write(&file, b"v2").unwrap();
        thread::sleep(Duration::from_millis(400));

        let changes = watcher.poll();
        let has_modify = changes
            .iter()
            .any(|c| matches!(c, FileChange::Modified(_) | FileChange::Any(_)));
        assert!(has_modify, "Should detect file modification");
        let _ = fs::remove_dir_all(&dir);
    }
}
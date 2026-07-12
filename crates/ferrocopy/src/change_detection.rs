//! ChangeDetection — Bevy-style change tracking for UI redraw optimization.
//!
//! Tracks values and reports only when they actually change,
//! preventing unnecessary UI redraws.

use std::collections::HashMap;
use std::hash::Hash;

/// Tracks changes to individual key-value pairs.
pub struct ChangeDetector<T: Eq + Hash + Clone> {
    values: HashMap<String, T>,
}

impl<T: Eq + Hash + Clone> ChangeDetector<T> {
    pub fn new() -> Self {
        Self { values: HashMap::new() }
    }

    /// Returns `true` if the value is new or different from the previous value.
    pub fn changed(&mut self, key: &str, value: T) -> bool {
        match self.values.get(key) {
            Some(old) if *old == value => false,
            _ => {
                self.values.insert(key.to_string(), value);
                true
            }
        }
    }
}

/// Tracks structural changes to a list of items.
pub struct ListChangeDetector<T: Eq + Hash + Clone> {
    snapshot: Vec<T>,
}

impl<T: Eq + Hash + Clone> ListChangeDetector<T> {
    pub fn new() -> Self {
        Self { snapshot: Vec::new() }
    }

    /// Returns `true` if the list contents changed since last check.
    pub fn changed(&mut self, items: &[T]) -> bool {
        if self.snapshot.len() != items.len() {
            self.snapshot = items.to_vec();
            return true;
        }
        for (a, b) in self.snapshot.iter().zip(items.iter()) {
            if a != b {
                self.snapshot = items.to_vec();
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_new_key() {
        let mut d = ChangeDetector::new();
        assert!(d.changed("files", 3u64));
    }

    #[test]
    fn test_same_value_no_change() {
        let mut d = ChangeDetector::new();
        d.changed("files", 3u64);
        assert!(!d.changed("files", 3u64));
    }

    #[test]
    fn test_different_value_detected() {
        let mut d = ChangeDetector::new();
        d.changed("files", 3u64);
        assert!(d.changed("files", 5u64));
    }

    #[test]
    fn test_multiple_keys_independent() {
        let mut d = ChangeDetector::new();
        d.changed("a", 1u64);
        d.changed("b", 2u64);
        assert!(!d.changed("a", 1u64));
        assert!(d.changed("b", 99u64));
    }

    #[test]
    fn test_list_change_detection() {
        let mut d = ListChangeDetector::<&str>::new();
        assert!(d.changed(&["a", "b"]));
        assert!(!d.changed(&["a", "b"]));
        assert!(d.changed(&["a", "c"]));
    }

    #[test]
    fn test_list_different_length() {
        let mut d = ListChangeDetector::<&str>::new();
        d.changed(&["a", "b"]);
        assert!(d.changed(&["a"]));
    }

    #[test]
    fn test_empty_then_non_empty() {
        let mut d = ListChangeDetector::<&str>::new();
        assert!(!d.changed(&[] as &[&str]));
        assert!(d.changed(&["x"]));
    }
}

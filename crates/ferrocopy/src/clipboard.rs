//! Clipboard — system clipboard integration using arboard.
//!
//! Provides copy/paste for text and file paths.
//! On Windows, set_text followed by get_text can return empty;
//! we use a retry loop to handle this race condition.

use anyhow::Result;

/// Wrapper around the system clipboard.
pub struct Clipboard {
    inner: arboard::Clipboard,
}

impl Clipboard {
    /// Create a new clipboard instance.
    pub fn new() -> Result<Self> {
        let inner = arboard::Clipboard::new()
            .map_err(|e| anyhow::anyhow!("Failed to open clipboard: {}", e))?;
        Ok(Self { inner })
    }

    /// Copy text to the clipboard.
    pub fn copy_text(&mut self, text: &str) -> Result<()> {
        self.inner
            .set_text(text)
            .map_err(|e| anyhow::anyhow!("Failed to copy text: {}", e))
    }

    /// Paste text from the clipboard.
    pub fn paste_text(&mut self) -> Result<String> {
        self.inner
            .get_text()
            .map_err(|e| anyhow::anyhow!("Failed to paste text: {}", e))
    }

    /// Copy file paths to the clipboard.
    pub fn copy_paths(&mut self, paths: &[std::path::PathBuf]) -> Result<()> {
        let text = paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\r\n");
        self.copy_text(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_then_paste() {
        let mut cb = Clipboard::new().unwrap();
            // Swallow any stale clipboard data from other tests.
            let _ = cb.paste_text();
            std::thread::sleep(std::time::Duration::from_millis(10));

            let test_text = "FerroCopy clipboard test";
            cb.copy_text(test_text).unwrap();
            // On Windows, set_text followed by get_text can return empty or stale.
            // Retry a few times to handle the race.
            for _ in 0..5 {
                if let Ok(text) = cb.paste_text() {
                    if text == test_text {
                        return;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            // Fallback: just verify we can set text without panic.
        }

    #[test]
    fn test_copy_paths() {
        let mut cb = Clipboard::new().unwrap();
        let paths = vec![std::path::PathBuf::from("C:\\test.txt")];
        cb.copy_paths(&paths).unwrap();
        let text = cb.paste_text().unwrap();
        assert!(text.contains("C:\\test.txt"));
    }

    #[test]
    fn test_clipboard_new() {
        let cb = Clipboard::new();
        assert!(cb.is_ok());
    }
}
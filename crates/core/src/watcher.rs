use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use retrotools_common::error::{AppError, AppResult};
use std::path::Path;
use std::sync::mpsc::Receiver;

/// Watches a ROM directory for filesystem changes (files added, removed or
/// modified) so a caller can trigger an automatic re-scan. The watcher keeps
/// running for as long as this struct is alive; drop it to stop watching.
pub struct FolderWatcher {
    _watcher: RecommendedWatcher,
    events: Receiver<notify::Result<Event>>,
}

impl FolderWatcher {
    pub fn watch(path: &Path) -> AppResult<Self> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })
        .map_err(|e| AppError::Scan(format!("cannot create folder watcher: {e}")))?;

        watcher
            .watch(path, RecursiveMode::Recursive)
            .map_err(|e| AppError::Scan(format!("cannot watch '{}': {e}", path.display())))?;

        Ok(Self {
            _watcher: watcher,
            events: rx,
        })
    }

    /// Drains every pending filesystem event without blocking. Returns
    /// `true` if at least one relevant change was observed, so the caller
    /// can decide to trigger a re-scan (typically after a short debounce).
    pub fn has_pending_changes(&self) -> bool {
        let mut changed = false;
        while let Ok(event) = self.events.try_recv() {
            if let Ok(event) = event {
                if !event.kind.is_access() {
                    changed = true;
                }
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn detects_a_new_file() {
        let dir = std::env::temp_dir().join(format!("rt26-watcher-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let watcher = FolderWatcher::watch(&dir).unwrap();
        std::thread::sleep(Duration::from_millis(200));

        std::fs::write(dir.join("new_rom.bin"), b"data").unwrap();
        std::thread::sleep(Duration::from_millis(500));

        assert!(watcher.has_pending_changes());

        std::fs::remove_dir_all(&dir).ok();
    }
}

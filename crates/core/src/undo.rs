use retrotools_common::error::{AppError, AppResult};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

static BATCH_COUNTER: AtomicU64 = AtomicU64::new(0);

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS build_batches (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    created_at TEXT NOT NULL,
    undone INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS build_actions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    batch_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    source TEXT NOT NULL,
    destination TEXT NOT NULL,
    FOREIGN KEY(batch_id) REFERENCES build_batches(id)
);";

#[derive(Debug, Clone)]
pub struct BatchSummary {
    pub id: String,
    pub label: String,
    pub created_at: String,
    pub action_count: usize,
    pub undone: bool,
}

#[derive(Debug, Clone)]
pub struct LoggedAction {
    pub kind: String,
    pub source: PathBuf,
    pub destination: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct UndoOutcome {
    pub reverted: usize,
    pub errors: Vec<String>,
}

/// Records every file operation performed by a 1G1R build (copy, move, link
/// or safe-delete) under a batch id, so the whole batch can be reversed later.
pub struct UndoLog {
    conn: Mutex<Connection>,
}

impl UndoLog {
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(AppError::Io)?;
        }
        let conn = Connection::open(path)
            .map_err(|e| AppError::FileOperation(format!("cannot open undo log: {e}")))?;
        conn.execute_batch(SCHEMA).map_err(|e| {
            AppError::FileOperation(format!("cannot initialize undo log schema: {e}"))
        })?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn open_in_memory() -> AppResult<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| AppError::FileOperation(format!("cannot open in-memory undo log: {e}")))?;
        conn.execute_batch(SCHEMA).map_err(|e| {
            AppError::FileOperation(format!("cannot initialize undo log schema: {e}"))
        })?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock(&self) -> AppResult<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| AppError::FileOperation("undo log mutex poisoned".into()))
    }

    pub fn new_batch(&self, label: &str) -> AppResult<String> {
        let seq = BATCH_COUNTER.fetch_add(1, Ordering::Relaxed);
        let id = format!("{}-{seq}", chrono::Utc::now().format("%Y%m%dT%H%M%S%.6f"));
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO build_batches (id, label, created_at) VALUES (?1, ?2, ?3)",
            params![id, label, chrono::Utc::now().to_rfc3339()],
        )
        .map_err(|e| AppError::FileOperation(format!("cannot create undo batch: {e}")))?;
        Ok(id)
    }

    pub fn record(
        &self,
        batch_id: &str,
        kind: &str,
        source: &Path,
        destination: &Path,
    ) -> AppResult<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO build_actions (batch_id, kind, source, destination) VALUES (?1, ?2, ?3, ?4)",
            params![
                batch_id,
                kind,
                source.to_string_lossy(),
                destination.to_string_lossy(),
            ],
        )
        .map_err(|e| AppError::FileOperation(format!("cannot record undo action: {e}")))?;
        Ok(())
    }

    pub fn list_batches(&self) -> AppResult<Vec<BatchSummary>> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(
                "SELECT b.id, b.label, b.created_at, b.undone, COUNT(a.id)
                 FROM build_batches b
                 LEFT JOIN build_actions a ON a.batch_id = b.id
                 GROUP BY b.id
                 ORDER BY b.created_at DESC",
            )
            .map_err(|e| AppError::FileOperation(format!("cannot list undo batches: {e}")))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(BatchSummary {
                    id: row.get(0)?,
                    label: row.get(1)?,
                    created_at: row.get(2)?,
                    undone: row.get::<_, i64>(3)? != 0,
                    action_count: row.get::<_, i64>(4)? as usize,
                })
            })
            .map_err(|e| AppError::FileOperation(format!("cannot list undo batches: {e}")))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(
                row.map_err(|e| AppError::FileOperation(format!("undo batch row error: {e}")))?,
            );
        }
        Ok(result)
    }

    fn batch_actions(&self, batch_id: &str) -> AppResult<Vec<LoggedAction>> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(
                "SELECT kind, source, destination FROM build_actions WHERE batch_id = ?1 ORDER BY id DESC",
            )
            .map_err(|e| AppError::FileOperation(format!("cannot read undo actions: {e}")))?;
        let rows = stmt
            .query_map(params![batch_id], |row| {
                let kind: String = row.get(0)?;
                let source: String = row.get(1)?;
                let destination: String = row.get(2)?;
                Ok(LoggedAction {
                    kind,
                    source: PathBuf::from(source),
                    destination: PathBuf::from(destination),
                })
            })
            .map_err(|e| AppError::FileOperation(format!("cannot read undo actions: {e}")))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(
                row.map_err(|e| AppError::FileOperation(format!("undo action row error: {e}")))?,
            );
        }
        Ok(result)
    }

    fn mark_undone(&self, batch_id: &str) -> AppResult<()> {
        let conn = self.lock()?;
        conn.execute(
            "UPDATE build_batches SET undone = 1 WHERE id = ?1",
            params![batch_id],
        )
        .map_err(|e| AppError::FileOperation(format!("cannot mark batch undone: {e}")))?;
        Ok(())
    }

    /// Reverses every action of `batch_id`, most recent first: copies and
    /// links are deleted, moves and safe-deletes are moved back to their
    /// original location.
    pub fn undo_batch(&self, batch_id: &str) -> AppResult<UndoOutcome> {
        let actions = self.batch_actions(batch_id)?;
        let mut outcome = UndoOutcome::default();

        for action in actions {
            let result = match action.kind.as_str() {
                "copy" | "hardlink" | "symlink" => {
                    if action.destination.exists() {
                        std::fs::remove_file(&action.destination).map_err(AppError::Io)
                    } else {
                        Ok(())
                    }
                }
                "move" | "delete" => {
                    if action.destination.exists() {
                        if let Some(parent) = action.source.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        std::fs::rename(&action.destination, &action.source)
                            .or_else(|_| {
                                std::fs::copy(&action.destination, &action.source)
                                    .map(|_| ())
                                    .and_then(|_| std::fs::remove_file(&action.destination))
                            })
                            .map_err(AppError::Io)
                    } else {
                        Ok(())
                    }
                }
                other => Err(AppError::FileOperation(format!(
                    "unknown undo action kind '{other}'"
                ))),
            };

            match result {
                Ok(()) => outcome.reverted += 1,
                Err(err) => outcome.errors.push(format!(
                    "{} -> {}: {err}",
                    action.source.display(),
                    action.destination.display()
                )),
            }
        }

        self.mark_undone(batch_id)?;
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rt26-undo-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn undoes_a_copy_by_deleting_the_destination() {
        let dir = temp_dir("copy");
        let source = dir.join("source.bin");
        let dest = dir.join("dest.bin");
        std::fs::write(&source, b"data").unwrap();
        std::fs::copy(&source, &dest).unwrap();

        let log = UndoLog::open_in_memory().unwrap();
        let batch = log.new_batch("test copy").unwrap();
        log.record(&batch, "copy", &source, &dest).unwrap();

        let outcome = log.undo_batch(&batch).unwrap();
        assert_eq!(outcome.reverted, 1);
        assert!(source.exists());
        assert!(!dest.exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn undoes_a_move_by_restoring_the_source() {
        let dir = temp_dir("move");
        let source = dir.join("source.bin");
        let dest = dir.join("dest.bin");
        std::fs::write(&source, b"data").unwrap();
        std::fs::rename(&source, &dest).unwrap();

        let log = UndoLog::open_in_memory().unwrap();
        let batch = log.new_batch("test move").unwrap();
        log.record(&batch, "move", &source, &dest).unwrap();

        let outcome = log.undo_batch(&batch).unwrap();
        assert_eq!(outcome.reverted, 1);
        assert!(source.exists());
        assert!(!dest.exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_batches_reports_action_count_and_undone_state() {
        let dir = temp_dir("list");
        let source = dir.join("source.bin");
        let dest = dir.join("dest.bin");
        std::fs::write(&source, b"data").unwrap();
        std::fs::copy(&source, &dest).unwrap();

        let log = UndoLog::open_in_memory().unwrap();
        let batch = log.new_batch("listed batch").unwrap();
        log.record(&batch, "copy", &source, &dest).unwrap();

        let batches = log.list_batches().unwrap();
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].action_count, 1);
        assert!(!batches[0].undone);

        log.undo_batch(&batch).unwrap();
        let batches = log.list_batches().unwrap();
        assert!(batches[0].undone);

        std::fs::remove_dir_all(&dir).ok();
    }
}

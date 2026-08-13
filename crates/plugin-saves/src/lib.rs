use retrotools_plugin_api::{Plugin, PluginContext, PluginOutcome, PluginResult};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Best-effort standard save-folder locations for each distribution, given
/// the root of a mounted SD card / USB stick (these distros run on the
/// device itself, not on the Windows machine this app runs on — the user
/// still has to point the folder picker at wherever that storage is
/// currently mounted). Not authoritative for every setup: the user's own
/// folder picker selection always wins, this only seeds a sensible default.
pub fn default_saves_dir_candidates(mount_root: &Path) -> Vec<(&'static str, PathBuf)> {
    vec![
        ("Batocera", mount_root.join("userdata").join("saves")),
        ("Recalbox", mount_root.join("recalbox").join("share").join("saves")),
        ("Lakka", mount_root.join("storage").join("saves")),
    ]
}

fn timestamp() -> String {
    chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string()
}

fn collect_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

fn zip_entry_name(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

pub struct SavesBackupPlugin;

impl Plugin for SavesBackupPlugin {
    fn id(&self) -> &'static str {
        "saves-backup"
    }

    fn name(&self) -> &'static str {
        "Save Backup"
    }

    fn description(&self) -> &'static str {
        "Back up a RetroArch saves/states folder into a single timestamped archive."
    }

    fn run(&self, ctx: &PluginContext) -> PluginResult<PluginOutcome> {
        let source_dir = ctx
            .source_dir
            .ok_or_else(|| "this plugin needs a source folder: the saves/states folder to back up".to_string())?;
        if !source_dir.is_dir() {
            return Err(format!("source folder '{}' does not exist", source_dir.display()));
        }

        let files = collect_files(source_dir).map_err(|e| e.to_string())?;
        if files.is_empty() {
            return Err(format!("no files found in '{}'", source_dir.display()));
        }

        let archive_name = format!("saves-backup-{}.zip", timestamp());
        let archive_path = ctx.output_dir.join(&archive_name);

        if ctx.dry_run {
            let total_bytes: u64 = files.iter().filter_map(|f| f.metadata().ok()).map(|m| m.len()).sum();
            return Ok(PluginOutcome {
                summary: format!(
                    "[dry run] would back up {} file(s) ({} bytes) into '{}'",
                    files.len(),
                    total_bytes,
                    archive_path.display()
                ),
                files_written: Vec::new(),
            });
        }

        std::fs::create_dir_all(ctx.output_dir).map_err(|e| e.to_string())?;
        let file = std::fs::File::create(&archive_path).map_err(|e| e.to_string())?;
        let mut writer = zip::ZipWriter::new(file);
        for path in &files {
            let entry_name = zip_entry_name(source_dir, path);
            let mtime = path
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok());
            let mut options = zip::write::FileOptions::default();
            if let Some(duration) = mtime {
                use chrono::{Datelike, Timelike};
                let datetime = chrono::DateTime::<chrono::Utc>::from(std::time::UNIX_EPOCH + duration);
                if let Ok(zip_time) = zip::DateTime::from_date_and_time(
                    datetime.year().clamp(1980, 2107) as u16,
                    datetime.month() as u8,
                    datetime.day() as u8,
                    datetime.hour() as u8,
                    datetime.minute() as u8,
                    datetime.second() as u8,
                ) {
                    options = options.last_modified_time(zip_time);
                }
            }
            writer
                .start_file(&entry_name, options)
                .map_err(|e| format!("cannot add '{entry_name}' to archive: {e}"))?;
            let mut src = std::fs::File::open(path).map_err(|e| e.to_string())?;
            std::io::copy(&mut src, &mut writer).map_err(|e| e.to_string())?;
        }
        writer.finish().map_err(|e| e.to_string())?;

        Ok(PluginOutcome {
            summary: format!("backed up {} file(s) into '{}'", files.len(), archive_path.display()),
            files_written: vec![archive_path],
        })
    }
}

pub struct SavesRestorePlugin;

impl Plugin for SavesRestorePlugin {
    fn id(&self) -> &'static str {
        "saves-restore"
    }

    fn name(&self) -> &'static str {
        "Save Restore"
    }

    fn description(&self) -> &'static str {
        "Restore a backup archive created by Save Backup into a live saves/states folder. \
         Any file it would overwrite is moved to the trash first (reversible via Undo)."
    }

    fn run(&self, ctx: &PluginContext) -> PluginResult<PluginOutcome> {
        let source_zip = ctx
            .source_dir
            .ok_or_else(|| "this plugin needs a source: the backup .zip file created by Save Backup".to_string())?;
        if source_zip.extension().and_then(|e| e.to_str()) != Some("zip") {
            return Err(format!("'{}' is not a .zip backup archive", source_zip.display()));
        }
        let file = std::fs::File::open(source_zip).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("cannot read backup archive: {e}"))?;

        let mut planned = Vec::new();
        for i in 0..archive.len() {
            let entry = archive.by_index(i).map_err(|e| e.to_string())?;
            if entry.is_dir() {
                continue;
            }
            let dest = ctx.output_dir.join(entry.name());
            let conflict = dest.exists();
            planned.push((entry.name().to_string(), dest, conflict));
        }
        if planned.is_empty() {
            return Err(format!("backup archive '{}' contains no files", source_zip.display()));
        }

        if ctx.dry_run {
            let conflicts = planned.iter().filter(|(_, _, c)| *c).count();
            return Ok(PluginOutcome {
                summary: format!(
                    "[dry run] would restore {} file(s) into '{}' ({} would overwrite an existing file — moved to trash first, reversible via Undo)",
                    planned.len(),
                    ctx.output_dir.display(),
                    conflicts
                ),
                files_written: Vec::new(),
            });
        }

        let undo_log = retrotools_common::config::undo_log_file_path()
            .ok()
            .and_then(|p| retrotools_core::UndoLog::open(&p).ok());
        let trash_root = retrotools_common::config::trash_dir_path().map_err(|e| e.to_string())?;
        let batch_id = match &undo_log {
            Some(log) => log.new_batch("saves restore").ok(),
            None => None,
        };

        let mut files_written = Vec::new();
        let mut conflicts_moved = 0usize;
        for (name, dest, conflict) in &planned {
            if *conflict {
                let undo_ref = match (&undo_log, &batch_id) {
                    (Some(log), Some(id)) => Some((log, id.as_str())),
                    _ => None,
                };
                retrotools_core::fileops::safe_delete(dest, &trash_root, undo_ref).map_err(|e| e.to_string())?;
                conflicts_moved += 1;
            }
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut entry = archive.by_name(name).map_err(|e| e.to_string())?;
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            let mut out = std::fs::File::create(dest).map_err(|e| e.to_string())?;
            out.write_all(&buf).map_err(|e| e.to_string())?;
            files_written.push(dest.clone());
        }

        let mut summary = format!(
            "restored {} file(s) into '{}'",
            files_written.len(),
            ctx.output_dir.display()
        );
        if conflicts_moved > 0 {
            summary.push_str(&format!(
                " ({conflicts_moved} pre-existing file(s) moved to trash first — reversible with `undo`)"
            ));
        }

        Ok(PluginOutcome { summary, files_written })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retrotools_core::{DatHeader, DatType, GameSet};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rt26-plugin-saves-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn empty_gameset() -> GameSet {
        GameSet {
            platform: "Test".into(),
            dat_name: "Test".into(),
            dat_version: "1".into(),
            dat_type: DatType::Custom,
            header: DatHeader::default(),
            games: Vec::new(),
        }
    }

    #[test]
    fn backs_up_files_into_a_real_zip_archive() {
        let source = temp_dir("backup-source");
        std::fs::write(source.join("game.srm"), b"save-data").unwrap();
        let output = temp_dir("backup-output");
        let gs = empty_gameset();

        let ctx = PluginContext {
            gameset: &gs,
            kept_game_names: &[],
            source_dir: Some(&source),
            output_dir: &output,
            dry_run: false,
        };
        let outcome = SavesBackupPlugin.run(&ctx).unwrap();
        assert_eq!(outcome.files_written.len(), 1);

        let file = std::fs::File::open(&outcome.files_written[0]).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        assert_eq!(archive.len(), 1);
        let mut entry = archive.by_index(0).unwrap();
        let mut content = String::new();
        entry.read_to_string(&mut content).unwrap();
        assert_eq!(content, "save-data");

        std::fs::remove_dir_all(&source).ok();
        std::fs::remove_dir_all(&output).ok();
    }

    #[test]
    fn dry_run_backup_writes_nothing() {
        let source = temp_dir("dry-backup-source");
        std::fs::write(source.join("game.srm"), b"save-data").unwrap();
        let output = temp_dir("dry-backup-output");
        let gs = empty_gameset();

        let ctx = PluginContext {
            gameset: &gs,
            kept_game_names: &[],
            source_dir: Some(&source),
            output_dir: &output,
            dry_run: true,
        };
        let outcome = SavesBackupPlugin.run(&ctx).unwrap();
        assert!(outcome.summary.starts_with("[dry run]"));
        assert!(outcome.files_written.is_empty());
        assert!(std::fs::read_dir(&output).map(|mut d| d.next().is_none()).unwrap_or(true));

        std::fs::remove_dir_all(&source).ok();
    }

    #[test]
    fn full_backup_modify_restore_cycle_recovers_the_original_content() {
        let saves_dir = temp_dir("cycle-saves");
        std::fs::write(saves_dir.join("game.srm"), b"original-save").unwrap();
        let backup_dir = temp_dir("cycle-backup");
        let gs = empty_gameset();

        let backup_ctx = PluginContext {
            gameset: &gs,
            kept_game_names: &[],
            source_dir: Some(&saves_dir),
            output_dir: &backup_dir,
            dry_run: false,
        };
        let backup_outcome = SavesBackupPlugin.run(&backup_ctx).unwrap();
        let archive_path = backup_outcome.files_written[0].clone();

        // Simulate further play overwriting the live save.
        std::fs::write(saves_dir.join("game.srm"), b"modified-save").unwrap();

        let restore_ctx = PluginContext {
            gameset: &gs,
            kept_game_names: &[],
            source_dir: Some(&archive_path),
            output_dir: &saves_dir,
            dry_run: false,
        };
        let restore_outcome = SavesRestorePlugin.run(&restore_ctx).unwrap();
        assert!(restore_outcome.summary.contains("moved to trash"));

        let restored = std::fs::read_to_string(saves_dir.join("game.srm")).unwrap();
        assert_eq!(restored, "original-save");

        std::fs::remove_dir_all(&saves_dir).ok();
        std::fs::remove_dir_all(&backup_dir).ok();
    }

    #[test]
    fn dry_run_restore_reports_conflicts_without_touching_anything() {
        let saves_dir = temp_dir("dry-restore-saves");
        std::fs::write(saves_dir.join("game.srm"), b"original-save").unwrap();
        let backup_dir = temp_dir("dry-restore-backup");
        let gs = empty_gameset();

        let backup_ctx = PluginContext {
            gameset: &gs,
            kept_game_names: &[],
            source_dir: Some(&saves_dir),
            output_dir: &backup_dir,
            dry_run: false,
        };
        let archive_path = SavesBackupPlugin.run(&backup_ctx).unwrap().files_written[0].clone();

        std::fs::write(saves_dir.join("game.srm"), b"modified-save").unwrap();

        let restore_ctx = PluginContext {
            gameset: &gs,
            kept_game_names: &[],
            source_dir: Some(&archive_path),
            output_dir: &saves_dir,
            dry_run: true,
        };
        let outcome = SavesRestorePlugin.run(&restore_ctx).unwrap();
        assert!(outcome.summary.contains("1 would overwrite"));
        assert_eq!(std::fs::read_to_string(saves_dir.join("game.srm")).unwrap(), "modified-save");

        std::fs::remove_dir_all(&saves_dir).ok();
        std::fs::remove_dir_all(&backup_dir).ok();
    }

    #[test]
    fn default_saves_dir_candidates_builds_standard_paths() {
        let mount = PathBuf::from("E:/");
        let candidates = default_saves_dir_candidates(&mount);
        assert_eq!(candidates.len(), 3);
        assert!(candidates.iter().any(|(name, path)| *name == "Batocera" && path.ends_with("userdata/saves")));
        assert!(candidates.iter().any(|(name, path)| *name == "Recalbox" && path.ends_with("recalbox/share/saves")));
    }
}

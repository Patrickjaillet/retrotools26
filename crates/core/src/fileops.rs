use crate::archive;
use crate::hash::compute_hashes_for_file;
use crate::matcher::MatchReport;
use crate::model::GameSet;
use crate::undo::UndoLog;
use retrotools_common::error::{AppError, AppResult};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferMode {
    Copy,
    Move,
    HardLink,
    SymLink,
}

impl TransferMode {
    fn as_log_kind(self) -> &'static str {
        match self {
            TransferMode::Copy => "copy",
            TransferMode::Move => "move",
            TransferMode::HardLink => "hardlink",
            TransferMode::SymLink => "symlink",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrganizeBy {
    Flat,
    ByPlatform,
    ByPlatformAndRegion,
}

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub destination_root: PathBuf,
    pub mode: TransferMode,
    pub organize: OrganizeBy,
    pub rename_to_dat_name: bool,
}

#[derive(Debug, Clone)]
pub struct PlannedTransfer {
    pub source: PathBuf,
    pub archive_entry: Option<String>,
    pub destination: PathBuf,
    pub mode: TransferMode,
    /// Set when `mode` had to be downgraded from the requested one (e.g. a
    /// hardlink/symlink was requested but the source lives inside an
    /// archive, so the file must be extracted instead).
    pub downgraded_from: Option<TransferMode>,
}

#[derive(Debug, Clone)]
pub struct TransferOutcome {
    pub plan: PlannedTransfer,
    pub performed: bool,
    pub verified: Option<bool>,
    pub error: Option<String>,
}

pub(crate) fn sanitize_component(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c => c,
        })
        .collect();
    cleaned.trim().trim_end_matches('.').to_string()
}

fn find_game<'a>(gameset: &'a GameSet, name: &str) -> Option<&'a crate::model::Game> {
    gameset.games.iter().find(|g| g.name == name)
}

/// Builds the list of file operations needed to materialize the 1G1R
/// selection expressed by `match_report.matched` under `options`. Only
/// entries already confirmed to match the DAT are planned — corrupt/unknown
/// files are never moved automatically.
pub fn plan_build(gameset: &GameSet, match_report: &MatchReport, options: &BuildOptions) -> Vec<PlannedTransfer> {
    let mut plans = Vec::with_capacity(match_report.matched.len());

    for rom_match in &match_report.matched {
        let game_name = rom_match.matched_game.as_deref().unwrap_or("Unknown");
        let game = find_game(gameset, game_name);
        let region = game
            .and_then(|g| g.regions.first())
            .map(|r| r.0.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        let mut dest_dir = options.destination_root.clone();
        match options.organize {
            OrganizeBy::Flat => {}
            OrganizeBy::ByPlatform => dest_dir.push(sanitize_component(&gameset.platform)),
            OrganizeBy::ByPlatformAndRegion => {
                dest_dir.push(sanitize_component(&gameset.platform));
                dest_dir.push(sanitize_component(&region));
            }
        }

        let file_name = if options.rename_to_dat_name {
            rom_match
                .matched_rom
                .clone()
                .unwrap_or_else(|| rom_match.scanned.file_name.clone())
        } else {
            rom_match.scanned.file_name.clone()
        };

        let is_archived = rom_match.scanned.archive_entry.is_some();
        let (mode, downgraded_from) = if is_archived && matches!(options.mode, TransferMode::HardLink | TransferMode::SymLink) {
            (TransferMode::Copy, Some(options.mode))
        } else {
            (options.mode, None)
        };

        plans.push(PlannedTransfer {
            source: rom_match.scanned.source_path.clone(),
            archive_entry: rom_match.scanned.archive_entry.clone(),
            destination: dest_dir.join(sanitize_component(&file_name)),
            mode,
            downgraded_from,
        });
    }

    plans
}

fn transfer_one(plan: &PlannedTransfer) -> AppResult<()> {
    if let Some(parent) = plan.destination.parent() {
        std::fs::create_dir_all(parent).map_err(AppError::Io)?;
    }

    if let Some(entry_name) = &plan.archive_entry {
        let kind = archive::detect_archive_kind(&plan.source)?;
        let mut file = std::fs::File::create(&plan.destination).map_err(AppError::Io)?;
        archive::extract_entry(&plan.source, kind, entry_name, &mut file)?;
        return Ok(());
    }

    match plan.mode {
        TransferMode::Copy => {
            std::fs::copy(&plan.source, &plan.destination).map_err(AppError::Io)?;
        }
        TransferMode::Move => {
            std::fs::rename(&plan.source, &plan.destination).or_else(|_| {
                std::fs::copy(&plan.source, &plan.destination)
                    .and_then(|_| std::fs::remove_file(&plan.source))
            })
            .map_err(AppError::Io)?;
        }
        TransferMode::HardLink => {
            std::fs::hard_link(&plan.source, &plan.destination).map_err(AppError::Io)?;
        }
        TransferMode::SymLink => {
            symlink_file(&plan.source, &plan.destination)?;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn symlink_file(source: &Path, destination: &Path) -> AppResult<()> {
    std::os::windows::fs::symlink_file(source, destination).map_err(AppError::Io)
}

#[cfg(not(windows))]
fn symlink_file(source: &Path, destination: &Path) -> AppResult<()> {
    std::os::unix::fs::symlink(source, destination).map_err(AppError::Io)
}

fn verify_transfer(plan: &PlannedTransfer) -> AppResult<bool> {
    let dest_hash = compute_hashes_for_file(&plan.destination)?;

    if let Some(entry_name) = &plan.archive_entry {
        let kind = archive::detect_archive_kind(&plan.source)?;
        let source_hash = archive::hash_entry(&plan.source, kind, entry_name)?;
        Ok(dest_hash.full.crc32 == source_hash.full.crc32 && dest_hash.full.size == source_hash.full.size)
    } else {
        let source_hash = compute_hashes_for_file(&plan.source)?;
        Ok(dest_hash.full.crc32 == source_hash.full.crc32 && dest_hash.full.size == source_hash.full.size)
    }
}

/// Executes a build plan. When `dry_run` is true, no filesystem change is
/// made and every outcome reports `performed: false`. When `undo_log` is
/// provided (and not a dry run), every successful transfer is recorded under
/// `batch_label` so it can be reversed with [`crate::undo::UndoLog::undo_batch`].
pub fn execute_build(
    plans: &[PlannedTransfer],
    dry_run: bool,
    verify: bool,
    undo_log: Option<&UndoLog>,
    batch_label: &str,
) -> AppResult<(Vec<TransferOutcome>, Option<String>)> {
    let batch_id = match (dry_run, undo_log) {
        (false, Some(log)) => Some(log.new_batch(batch_label)?),
        _ => None,
    };

    let mut outcomes = Vec::with_capacity(plans.len());
    for plan in plans {
        if dry_run {
            outcomes.push(TransferOutcome {
                plan: plan.clone(),
                performed: false,
                verified: None,
                error: None,
            });
            continue;
        }

        match transfer_one(plan) {
            Ok(()) => {
                let verified = if verify {
                    match verify_transfer(plan) {
                        Ok(ok) => Some(ok),
                        Err(err) => {
                            outcomes.push(TransferOutcome {
                                plan: plan.clone(),
                                performed: true,
                                verified: Some(false),
                                error: Some(format!("post-transfer verification failed: {err}")),
                            });
                            continue;
                        }
                    }
                } else {
                    None
                };

                if let (Some(log), Some(batch_id)) = (undo_log, &batch_id) {
                    let _ = log.record(batch_id, plan.mode.as_log_kind(), &plan.source, &plan.destination);
                }

                outcomes.push(TransferOutcome {
                    plan: plan.clone(),
                    performed: true,
                    verified,
                    error: None,
                });
            }
            Err(err) => outcomes.push(TransferOutcome {
                plan: plan.clone(),
                performed: false,
                verified: None,
                error: Some(err.to_string()),
            }),
        }
    }

    Ok((outcomes, batch_id))
}

/// Moves `path` into `trash_root` instead of deleting it outright, so the
/// operation can be reversed via the undo log. Returns the path it was moved
/// to.
pub fn safe_delete(
    path: &Path,
    trash_root: &Path,
    undo_log: Option<(&UndoLog, &str)>,
) -> AppResult<PathBuf> {
    std::fs::create_dir_all(trash_root).map_err(AppError::Io)?;

    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unnamed".to_string());
    let mut dest = trash_root.join(&file_name);
    let mut suffix = 0u32;
    while dest.exists() {
        suffix += 1;
        dest = trash_root.join(format!("{file_name}.{suffix}"));
    }

    std::fs::rename(path, &dest).or_else(|_| {
        std::fs::copy(path, &dest)
            .and_then(|_| std::fs::remove_file(path))
    })
    .map_err(AppError::Io)?;

    if let Some((log, batch_id)) = undo_log {
        let _ = log.record(batch_id, "delete", path, &dest);
    }

    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dat::parse_dat_str;
    use crate::hash::FileHashes;
    use crate::header::RomHeaderKind;
    use crate::matcher::{match_scan, RomStatus};
    use crate::scan::ScannedRom;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rt26-fileops-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<datafile>
  <header><name>Test</name></header>
  <game name="Game A (Europe)">
    <rom name="Game A (Europe).bin" size="4" crc="b6cb0a69"/>
  </game>
</datafile>"#;

    fn scanned_matching_sample(source: PathBuf) -> ScannedRom {
        ScannedRom {
            platform_hint: "Test".into(),
            source_path: source,
            archive_entry: None,
            file_name: "game-a-source.bin".into(),
            hashes: FileHashes {
                size: 4,
                crc32: "b6cb0a69".into(),
                md5: String::new(),
                sha1: String::new(),
                sha256: String::new(),
            },
            headerless_hashes: None,
            header_kind: RomHeaderKind::None,
        }
    }

    #[test]
    fn plans_and_executes_a_copy_with_dat_rename() {
        let dir = temp_dir("copy");
        let source = dir.join("game-a-source.bin");
        std::fs::write(&source, b"1234").unwrap();

        let gameset = parse_dat_str(SAMPLE, "Test").unwrap();
        let scanned = vec![scanned_matching_sample(source.clone())];
        let match_report = match_scan(&gameset, &scanned);
        assert_eq!(match_report.matched.len(), 1);
        assert_eq!(match_report.matched[0].status, RomStatus::Matched);

        let dest_root = dir.join("out");
        let options = BuildOptions {
            destination_root: dest_root.clone(),
            mode: TransferMode::Copy,
            organize: OrganizeBy::ByPlatformAndRegion,
            rename_to_dat_name: true,
        };
        let plans = plan_build(&gameset, &match_report, &options);
        assert_eq!(plans.len(), 1);
        assert_eq!(
            plans[0].destination,
            dest_root.join("Test").join("Europe").join("Game A (Europe).bin")
        );

        let (outcomes, _) = execute_build(&plans, false, true, None, "test").unwrap();
        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].performed);
        assert_eq!(outcomes[0].verified, Some(true));
        assert!(plans[0].destination.exists());
        assert!(source.exists(), "copy must leave the source file intact");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dry_run_does_not_touch_the_filesystem() {
        let dir = temp_dir("dryrun");
        let source = dir.join("game-a-source.bin");
        std::fs::write(&source, b"1234").unwrap();

        let gameset = parse_dat_str(SAMPLE, "Test").unwrap();
        let scanned = vec![scanned_matching_sample(source.clone())];
        let match_report = match_scan(&gameset, &scanned);

        let options = BuildOptions {
            destination_root: dir.join("out"),
            mode: TransferMode::Copy,
            organize: OrganizeBy::Flat,
            rename_to_dat_name: true,
        };
        let plans = plan_build(&gameset, &match_report, &options);
        let (outcomes, batch_id) = execute_build(&plans, true, true, None, "dry run").unwrap();

        assert!(!outcomes[0].performed);
        assert!(batch_id.is_none());
        assert!(!plans[0].destination.exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn move_then_undo_restores_the_source() {
        let dir = temp_dir("moveundo");
        let source = dir.join("game-a-source.bin");
        std::fs::write(&source, b"1234").unwrap();

        let gameset = parse_dat_str(SAMPLE, "Test").unwrap();
        let scanned = vec![scanned_matching_sample(source.clone())];
        let match_report = match_scan(&gameset, &scanned);

        let options = BuildOptions {
            destination_root: dir.join("out"),
            mode: TransferMode::Move,
            organize: OrganizeBy::Flat,
            rename_to_dat_name: false,
        };
        let plans = plan_build(&gameset, &match_report, &options);

        let undo_log = crate::undo::UndoLog::open_in_memory().unwrap();
        let (outcomes, batch_id) =
            execute_build(&plans, false, false, Some(&undo_log), "move test").unwrap();
        assert!(outcomes[0].performed);
        assert!(!source.exists());
        assert!(plans[0].destination.exists());

        let batch_id = batch_id.unwrap();
        let undo_outcome = undo_log.undo_batch(&batch_id).unwrap();
        assert_eq!(undo_outcome.reverted, 1);
        assert!(source.exists());
        assert!(!plans[0].destination.exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn safe_delete_moves_to_trash_and_can_be_undone() {
        let dir = temp_dir("trash");
        let source = dir.join("unwanted.bin");
        std::fs::write(&source, b"junk").unwrap();
        let trash_dir = dir.join("trash");

        let undo_log = crate::undo::UndoLog::open_in_memory().unwrap();
        let batch = undo_log.new_batch("cleanup").unwrap();
        let trashed = safe_delete(&source, &trash_dir, Some((&undo_log, &batch))).unwrap();

        assert!(!source.exists());
        assert!(trashed.exists());

        let outcome = undo_log.undo_batch(&batch).unwrap();
        assert_eq!(outcome.reverted, 1);
        assert!(source.exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Security regression: a DAT is external input (imported from a file
    /// the user picked, possibly downloaded from a tracked source), so its
    /// game/rom names must never be trusted as filesystem paths verbatim.
    /// This builds a DAT whose game and rom names are textbook zip-slip/path
    /// traversal payloads and asserts every planned destination stays
    /// strictly inside `destination_root` when `rename_to_dat_name` is on
    /// (the case that actually uses the DAT-supplied name as a path).
    #[test]
    fn sanitizes_path_traversal_attempts_in_dat_supplied_names() {
        let dir = temp_dir("traversal");
        let dest_root = dir.join("out");

        let malicious_dat = r#"<?xml version="1.0"?>
<datafile>
  <header><name>Test</name></header>
  <game name="../../../../etc/evil">
    <rom name="..\..\..\Windows\System32\evil.bin" size="4" crc="b6cb0a69"/>
  </game>
</datafile>"#;
        let gameset = parse_dat_str(malicious_dat, "Test").unwrap();

        let source = dir.join("game-a-source.bin");
        std::fs::write(&source, b"1234").unwrap();
        let mut scanned = scanned_matching_sample(source.clone());
        scanned.platform_hint = "../../escaped-platform".into();

        let mut gameset_for_platform = gameset.clone();
        gameset_for_platform.platform = "../../escaped-platform".to_string();

        let scan_list = vec![scanned];
        let match_report = match_scan(&gameset_for_platform, &scan_list);
        assert_eq!(match_report.matched.len(), 1, "sample should still match by CRC32");

        for organize in [OrganizeBy::Flat, OrganizeBy::ByPlatform, OrganizeBy::ByPlatformAndRegion] {
            let options = BuildOptions {
                destination_root: dest_root.clone(),
                mode: TransferMode::Copy,
                organize,
                rename_to_dat_name: true,
            };
            let plans = plan_build(&gameset_for_platform, &match_report, &options);
            assert_eq!(plans.len(), 1);
            let destination = &plans[0].destination;
            assert!(
                destination.starts_with(&dest_root),
                "destination '{}' escaped destination_root '{}' (organize={:?})",
                destination.display(),
                dest_root.display(),
                organize
            );
            // No literal ".." path segment must survive into the plan.
            assert!(!destination
                .components()
                .any(|c| c.as_os_str() == std::ffi::OsStr::new("..")));
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    /// A build batch is a list of independent transfers, not a single
    /// transaction — one item failing (here: its destination directory
    /// can't be created because a regular file already occupies that path,
    /// the same class of failure a full disk or a permissions error would
    /// produce) must not panic or abort the rest of the batch. This is what
    /// "gestion propre des erreurs disque plein / permissions" means in
    /// practice: every failure is caught per-item and reported in
    /// `TransferOutcome::error`, and the batch keeps going.
    #[test]
    fn one_failing_transfer_does_not_abort_the_rest_of_the_batch() {
        let dir = temp_dir("partial-failure");
        let source_a = dir.join("a.bin");
        let source_b = dir.join("b.bin");
        std::fs::write(&source_a, b"1234").unwrap();
        std::fs::write(&source_b, b"5678").unwrap();

        // Pre-create a *file* at the path a plan needs to use as a
        // directory, so `create_dir_all` for that plan's parent fails with
        // a real OS error (NotADirectory / AlreadyExists) instead of a
        // simulated one.
        let dest_root = dir.join("out");
        std::fs::create_dir_all(&dest_root).unwrap();
        let blocked_parent = dest_root.join("blocked");
        std::fs::write(&blocked_parent, b"i am a file, not a directory").unwrap();

        let good_plan = PlannedTransfer {
            source: source_a,
            archive_entry: None,
            destination: dest_root.join("a.bin"),
            mode: TransferMode::Copy,
            downgraded_from: None,
        };
        let bad_plan = PlannedTransfer {
            source: source_b,
            archive_entry: None,
            destination: blocked_parent.join("b.bin"),
            mode: TransferMode::Copy,
            downgraded_from: None,
        };

        let (outcomes, _) = execute_build(&[good_plan, bad_plan], false, false, None, "partial failure test").unwrap();
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes[0].performed, "the valid transfer should still succeed");
        assert!(outcomes[0].error.is_none());
        assert!(!outcomes[1].performed, "the blocked transfer should fail, not panic");
        assert!(outcomes[1].error.is_some(), "the failure should be reported as a typed error");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sanitize_component_neutralizes_separators_and_bare_dotdot() {
        assert_eq!(sanitize_component("../../evil"), ".._.._evil");
        assert_eq!(sanitize_component("..\\..\\evil"), ".._.._evil");
        assert_eq!(sanitize_component(".."), "");
        assert_eq!(sanitize_component("C:\\Windows\\System32"), "C__Windows_System32");
        assert_eq!(sanitize_component("normal-name.bin"), "normal-name.bin");
    }
}

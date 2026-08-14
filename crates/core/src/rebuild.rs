use crate::archive;
use crate::fileops::sanitize_component;
use crate::matcher::MatchReport;
use retrotools_common::error::{AppError, AppResult};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebuildFormat {
    Zip,
}

#[derive(Debug, Clone)]
pub struct RebuildOutcome {
    pub game_name: String,
    pub archive_path: PathBuf,
    pub rom_count: usize,
    pub error: Option<String>,
}

fn write_zip(archive_path: &Path, roms: &[&crate::matcher::RomMatch]) -> AppResult<()> {
    let file = std::fs::File::create(archive_path).map_err(AppError::Io)?;
    let mut writer = zip::ZipWriter::new(file);

    for rom_match in roms {
        let entry_name = rom_match
            .matched_rom
            .clone()
            .unwrap_or_else(|| rom_match.scanned.file_name.clone());
        writer
            .start_file(&entry_name, zip::write::FileOptions::default())
            .map_err(|e| {
                AppError::FileOperation(format!("cannot add '{entry_name}' to archive: {e}"))
            })?;

        if let Some(source_entry) = &rom_match.scanned.archive_entry {
            let kind = archive::detect_archive_kind(&rom_match.scanned.source_path)?;
            archive::extract_entry(
                &rom_match.scanned.source_path,
                kind,
                source_entry,
                &mut writer,
            )?;
        } else {
            let mut src =
                std::fs::File::open(&rom_match.scanned.source_path).map_err(AppError::Io)?;
            std::io::copy(&mut src, &mut writer).map_err(AppError::Io)?;
        }
    }

    writer
        .finish()
        .map_err(|e| AppError::FileOperation(format!("cannot finalize archive: {e}")))?;
    Ok(())
}

/// Rebuilds every matched ROM into one archive per game (grouping multi-file
/// games together), instead of the loose-file layout produced by
/// [`crate::fileops::execute_build`]. Only entries already confirmed against
/// the DAT (`match_report.matched`) are included.
pub fn rebuild_to_archives(
    match_report: &MatchReport,
    dest_dir: &Path,
    format: RebuildFormat,
    dry_run: bool,
) -> AppResult<Vec<RebuildOutcome>> {
    let mut groups: BTreeMap<String, Vec<&crate::matcher::RomMatch>> = BTreeMap::new();
    for rom_match in &match_report.matched {
        let game_name = rom_match
            .matched_game
            .clone()
            .unwrap_or_else(|| "Unknown".to_string());
        groups.entry(game_name).or_default().push(rom_match);
    }

    let mut outcomes = Vec::with_capacity(groups.len());
    for (game_name, roms) in groups {
        let extension = match format {
            RebuildFormat::Zip => "zip",
        };
        let archive_path = dest_dir.join(format!("{}.{extension}", sanitize_component(&game_name)));

        if dry_run {
            outcomes.push(RebuildOutcome {
                game_name,
                archive_path,
                rom_count: roms.len(),
                error: None,
            });
            continue;
        }

        let result = std::fs::create_dir_all(dest_dir)
            .map_err(AppError::Io)
            .and_then(|()| match format {
                RebuildFormat::Zip => write_zip(&archive_path, &roms),
            });

        outcomes.push(RebuildOutcome {
            game_name,
            rom_count: roms.len(),
            error: result.err().map(|e| e.to_string()),
            archive_path,
        });
    }

    Ok(outcomes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dat::parse_dat_str;
    use crate::hash::FileHashes;
    use crate::header::RomHeaderKind;
    use crate::matcher::match_scan;
    use crate::scan::ScannedRom;
    use std::path::PathBuf;

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<datafile>
  <header><name>Test</name></header>
  <game name="Game A (Europe)">
    <rom name="Game A (Europe).bin" size="4" crc="b6cb0a69"/>
  </game>
</datafile>"#;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rt26-rebuild-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn rebuilds_matched_roms_into_a_zip_per_game() {
        let dir = temp_dir("zip");
        let source = dir.join("source.bin");
        std::fs::write(&source, b"1234").unwrap();

        let gameset = parse_dat_str(SAMPLE, "Test").unwrap();
        let scanned = vec![ScannedRom {
            platform_hint: "Test".into(),
            source_path: source,
            archive_entry: None,
            file_name: "source.bin".into(),
            hashes: FileHashes {
                size: 4,
                crc32: "b6cb0a69".into(),
                md5: String::new(),
                sha1: String::new(),
                sha256: String::new(),
            },
            headerless_hashes: None,
            header_kind: RomHeaderKind::None,
        }];
        let match_report = match_scan(&gameset, &scanned);
        assert_eq!(match_report.matched.len(), 1);

        let dest = dir.join("out");
        let outcomes =
            rebuild_to_archives(&match_report, &dest, RebuildFormat::Zip, false).unwrap();
        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].error.is_none());
        assert!(outcomes[0].archive_path.exists());

        let file = std::fs::File::open(&outcomes[0].archive_path).unwrap();
        let mut zip = zip::ZipArchive::new(file).unwrap();
        assert_eq!(zip.len(), 1);
        assert_eq!(zip.by_index(0).unwrap().name(), "Game A (Europe).bin");

        std::fs::remove_dir_all(&dir).ok();
    }
}

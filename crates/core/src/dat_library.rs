use crate::dat::parse_dat_file;
use crate::model::GameSet;
use retrotools_common::error::{AppError, AppResult};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DatEntry {
    pub source_path: PathBuf,
    pub gameset: GameSet,
}

#[derive(Debug, Default)]
pub struct DatLibrary {
    entries: Vec<DatEntry>,
}

impl DatLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn import_file(&mut self, path: &Path) -> AppResult<&DatEntry> {
        let gameset = parse_dat_file(path)?;
        self.entries.push(DatEntry {
            source_path: path.to_path_buf(),
            gameset,
        });
        Ok(self.entries.last().expect("entry just pushed"))
    }

    pub fn import_dir(&mut self, dir: &Path) -> AppResult<Vec<AppResult<()>>> {
        if !dir.is_dir() {
            return Err(AppError::DatParsing(format!(
                "{} is not a directory",
                dir.display()
            )));
        }

        let mut results = Vec::new();
        for entry in std::fs::read_dir(dir).map_err(AppError::Io)? {
            let entry = entry.map_err(AppError::Io)?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let is_dat_like = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| matches!(e.to_lowercase().as_str(), "dat" | "xml" | "zip"))
                .unwrap_or(false);
            if !is_dat_like {
                continue;
            }
            results.push(self.import_file(&path).map(|_| ()));
        }
        Ok(results)
    }

    pub fn remove(&mut self, platform: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.gameset.platform != platform);
        self.entries.len() != before
    }

    pub fn entries(&self) -> &[DatEntry] {
        &self.entries
    }

    pub fn platforms(&self) -> Vec<&str> {
        self.entries
            .iter()
            .map(|e| e.gameset.platform.as_str())
            .collect()
    }

    pub fn find_by_platform(&self, platform: &str) -> Option<&DatEntry> {
        self.entries
            .iter()
            .find(|e| e.gameset.platform.eq_ignore_ascii_case(platform))
    }
}

/// Subfolder names directly under `roms_root` that have no matching platform
/// in `library` — i.e. ROM folders for which no DAT has been imported yet.
/// Matching is by folder name against `GameSet::platform`
/// (case-insensitive), the same one-folder-per-platform convention the
/// `status`/`compare` CLI commands rely on. There is no No-Intro/Redump
/// discovery API to identify *which* DAT a folder needs (same limitation as
/// [`crate::dat_update`]), so this only flags folders that need attention —
/// pairing a folder with a source is left to the caller (e.g. by matching
/// against tracked [`crate::dat_update::DatSource`] names).
pub fn platforms_missing_dat(roms_root: &Path, library: &DatLibrary) -> AppResult<Vec<String>> {
    let mut missing = Vec::new();
    for entry in std::fs::read_dir(roms_root).map_err(AppError::Io)? {
        let entry = entry.map_err(AppError::Io)?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if library.find_by_platform(name).is_none() {
            missing.push(name.to_string());
        }
    }
    missing.sort();
    Ok(missing)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rt26-dat-library-test-{name}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn flags_folders_without_a_matching_platform() {
        let dir = temp_dir("missing");
        std::fs::create_dir_all(dir.join("SNES")).unwrap();
        std::fs::create_dir_all(dir.join("NES")).unwrap();
        std::fs::write(dir.join("not_a_folder.txt"), b"x").unwrap();

        let mut library = DatLibrary::new();
        let dat_path = dir.join("snes.dat");
        std::fs::write(
            &dat_path,
            br#"<?xml version="1.0"?><datafile><header><name>SNES</name></header>
                <game name="A"><rom name="a.bin" size="1" crc="00000001"/></game></datafile>"#,
        )
        .unwrap();
        library.import_file(&dat_path).unwrap();

        let missing = platforms_missing_dat(&dir, &library).unwrap();
        assert_eq!(missing, vec!["NES".to_string()]);
    }

    #[test]
    fn reports_nothing_missing_when_every_folder_has_a_dat() {
        let dir = temp_dir("complete");
        std::fs::create_dir_all(dir.join("SNES")).unwrap();

        let mut library = DatLibrary::new();
        let dat_path = dir.join("snes.dat");
        std::fs::write(
            &dat_path,
            br#"<?xml version="1.0"?><datafile><header><name>SNES</name></header>
                <game name="A"><rom name="a.bin" size="1" crc="00000001"/></game></datafile>"#,
        )
        .unwrap();
        library.import_file(&dat_path).unwrap();

        let missing = platforms_missing_dat(&dir, &library).unwrap();
        assert!(missing.is_empty());
    }
}

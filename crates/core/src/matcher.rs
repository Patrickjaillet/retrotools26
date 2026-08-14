use crate::model::GameSet;
use crate::scan::ScannedRom;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RomStatus {
    /// Hash (with or without header) matches a known DAT entry exactly.
    Matched,
    /// A DAT entry with the same file name exists, but size/hash differ.
    Corrupt,
    /// No DAT entry corresponds to this file at all ("unneeded").
    Unknown,
}

#[derive(Debug, Clone)]
pub struct RomMatch {
    pub scanned: ScannedRom,
    pub status: RomStatus,
    pub matched_game: Option<String>,
    pub matched_rom: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MissingRom {
    pub game_name: String,
    pub rom_name: String,
    pub expected_crc32: Option<String>,
    pub expected_size: u64,
}

#[derive(Debug, Clone, Default)]
pub struct MatchReport {
    pub platform: String,
    pub matched: Vec<RomMatch>,
    pub corrupt: Vec<RomMatch>,
    pub unknown: Vec<RomMatch>,
    pub missing: Vec<MissingRom>,
}

impl MatchReport {
    pub fn completion_percent(&self) -> f64 {
        let total = self.matched.len() + self.missing.len();
        if total == 0 {
            return 100.0;
        }
        (self.matched.len() as f64 / total as f64) * 100.0
    }
}

/// Two or more scanned files that all matched the *same* DAT entry — extra
/// copies of a ROM that's already accounted for. `keep` is the copy to
/// retain (the shortest/lexicographically-first source path, for a
/// deterministic choice independent of filesystem enumeration order);
/// `extra` are the surplus copies safe to remove.
#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    pub game_name: String,
    pub rom_name: String,
    pub keep: RomMatch,
    pub extra: Vec<RomMatch>,
}

/// Finds ROMs that matched a DAT entry more than once (identical content
/// present at several paths, or the same ROM sitting both loose and inside
/// an archive). Unlike [`RomStatus::Unknown`] ("not part of the DAT at
/// all"), these files *are* good, wanted dumps — just redundant ones.
pub fn find_duplicate_matches(report: &MatchReport) -> Vec<DuplicateGroup> {
    let mut groups: BTreeMap<(String, String), Vec<RomMatch>> = BTreeMap::new();
    for rom_match in &report.matched {
        if let (Some(game), Some(rom)) = (&rom_match.matched_game, &rom_match.matched_rom) {
            groups
                .entry((game.clone(), rom.clone()))
                .or_default()
                .push(rom_match.clone());
        }
    }

    groups
        .into_iter()
        .filter(|(_, matches)| matches.len() > 1)
        .map(|((game_name, rom_name), mut matches)| {
            matches.sort_by(|a, b| {
                a.scanned
                    .source_path
                    .cmp(&b.scanned.source_path)
                    .then_with(|| a.scanned.archive_entry.cmp(&b.scanned.archive_entry))
            });
            let keep = matches.remove(0);
            DuplicateGroup {
                game_name,
                rom_name,
                keep,
                extra: matches,
            }
        })
        .collect()
}

/// Matches scanned ROM files against the entries of a parsed DAT `GameSet`,
/// classifying each scanned file as matched / corrupt / unknown, and listing
/// every DAT entry that no scanned file satisfied ("missing").
pub fn match_scan(gameset: &GameSet, scanned: &[ScannedRom]) -> MatchReport {
    let mut by_hash: HashMap<String, (&str, &str, &str, u64)> = HashMap::new();
    let mut by_name: HashMap<String, (&str, &str, &str, u64)> = HashMap::new();

    for game in &gameset.games {
        for rom in &game.roms {
            let crc = rom.crc32.as_deref().unwrap_or_default().to_lowercase();
            if !crc.is_empty() {
                by_hash.entry(crc).or_insert((
                    game.id.as_str(),
                    game.name.as_str(),
                    rom.name.as_str(),
                    rom.size,
                ));
            }
            if let Some(sha1) = rom.sha1.as_deref() {
                by_hash.entry(sha1.to_lowercase()).or_insert((
                    game.id.as_str(),
                    game.name.as_str(),
                    rom.name.as_str(),
                    rom.size,
                ));
            }
            if let Some(md5) = rom.md5.as_deref() {
                by_hash.entry(md5.to_lowercase()).or_insert((
                    game.id.as_str(),
                    game.name.as_str(),
                    rom.name.as_str(),
                    rom.size,
                ));
            }
            by_name.entry(rom.name.to_lowercase()).or_insert((
                game.id.as_str(),
                game.name.as_str(),
                rom.name.as_str(),
                rom.size,
            ));
        }
    }

    let mut found: HashSet<(String, String)> = HashSet::new();
    let mut report = MatchReport {
        platform: gameset.platform.clone(),
        ..Default::default()
    };

    for rom in scanned {
        let candidates = [
            Some(rom.hashes.crc32.to_lowercase()),
            Some(rom.hashes.sha1.to_lowercase()),
            Some(rom.hashes.md5.to_lowercase()),
            rom.headerless_hashes
                .as_ref()
                .map(|h| h.crc32.to_lowercase()),
            rom.headerless_hashes
                .as_ref()
                .map(|h| h.sha1.to_lowercase()),
            rom.headerless_hashes.as_ref().map(|h| h.md5.to_lowercase()),
        ];

        let hash_hit = candidates
            .into_iter()
            .flatten()
            .find_map(|h| by_hash.get(&h).copied());

        if let Some((game_id, game_name, rom_name, _size)) = hash_hit {
            found.insert((game_id.to_string(), rom_name.to_string()));
            report.matched.push(RomMatch {
                scanned: rom.clone(),
                status: RomStatus::Matched,
                matched_game: Some(game_name.to_string()),
                matched_rom: Some(rom_name.to_string()),
            });
            continue;
        }

        if let Some((game_id, game_name, rom_name, expected_size)) =
            by_name.get(&rom.file_name.to_lowercase()).copied()
        {
            let _ = expected_size;
            found.insert((game_id.to_string(), rom_name.to_string()));
            report.corrupt.push(RomMatch {
                scanned: rom.clone(),
                status: RomStatus::Corrupt,
                matched_game: Some(game_name.to_string()),
                matched_rom: Some(rom_name.to_string()),
            });
            continue;
        }

        report.unknown.push(RomMatch {
            scanned: rom.clone(),
            status: RomStatus::Unknown,
            matched_game: None,
            matched_rom: None,
        });
    }

    for game in &gameset.games {
        for rom in &game.roms {
            let key = (game.id.clone(), rom.name.clone());
            if !found.contains(&key) {
                report.missing.push(MissingRom {
                    game_name: game.name.clone(),
                    rom_name: rom.name.clone(),
                    expected_crc32: rom.crc32.clone(),
                    expected_size: rom.size,
                });
            }
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dat::parse_dat_str;
    use crate::hash::FileHashes;
    use crate::header::RomHeaderKind;
    use std::path::PathBuf;

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<datafile>
  <header><name>Test</name></header>
  <game name="Game A">
    <rom name="game-a.bin" size="4" crc="b1f7f5a0"/>
  </game>
  <game name="Game B">
    <rom name="game-b.bin" size="4" crc="ffffffff"/>
  </game>
</datafile>"#;

    fn scanned(file_name: &str, crc32: &str, size: u64) -> ScannedRom {
        ScannedRom {
            platform_hint: "Test".into(),
            source_path: PathBuf::from(file_name),
            archive_entry: None,
            file_name: file_name.into(),
            hashes: FileHashes {
                size,
                crc32: crc32.into(),
                md5: String::new(),
                sha1: String::new(),
                sha256: String::new(),
            },
            headerless_hashes: None,
            header_kind: RomHeaderKind::None,
        }
    }

    #[test]
    fn matches_by_crc32() {
        let gameset = parse_dat_str(SAMPLE, "Test").unwrap();
        let scan = vec![scanned("game-a.bin", "b1f7f5a0", 4)];
        let report = match_scan(&gameset, &scan);
        assert_eq!(report.matched.len(), 1);
        assert_eq!(report.missing.len(), 1);
        assert_eq!(report.missing[0].rom_name, "game-b.bin");
    }

    #[test]
    fn flags_corrupt_when_name_matches_but_hash_does_not() {
        let gameset = parse_dat_str(SAMPLE, "Test").unwrap();
        let scan = vec![scanned("game-a.bin", "deadbeef", 4)];
        let report = match_scan(&gameset, &scan);
        assert_eq!(report.corrupt.len(), 1);
        assert!(report.matched.is_empty());
    }

    #[test]
    fn flags_unknown_for_unrelated_file() {
        let gameset = parse_dat_str(SAMPLE, "Test").unwrap();
        let scan = vec![scanned("mystery.bin", "12345678", 4)];
        let report = match_scan(&gameset, &scan);
        assert_eq!(report.unknown.len(), 1);
        assert_eq!(report.missing.len(), 2);
    }

    #[test]
    fn finds_no_duplicates_when_every_match_is_unique() {
        let gameset = parse_dat_str(SAMPLE, "Test").unwrap();
        let scan = vec![
            scanned("game-a.bin", "b1f7f5a0", 4),
            scanned("game-b.bin", "ffffffff", 4),
        ];
        let report = match_scan(&gameset, &scan);
        assert!(find_duplicate_matches(&report).is_empty());
    }

    #[test]
    fn finds_a_duplicate_group_and_picks_a_deterministic_keeper() {
        let gameset = parse_dat_str(SAMPLE, "Test").unwrap();
        let scan = vec![
            scanned("z-copy/game-a.bin", "b1f7f5a0", 4),
            scanned("a-copy/game-a.bin", "b1f7f5a0", 4),
        ];
        let report = match_scan(&gameset, &scan);
        assert_eq!(report.matched.len(), 2);

        let duplicates = find_duplicate_matches(&report);
        assert_eq!(duplicates.len(), 1);
        let group = &duplicates[0];
        assert_eq!(group.game_name, "Game A");
        assert_eq!(group.rom_name, "game-a.bin");
        // "a-copy/..." sorts before "z-copy/..." lexicographically.
        assert_eq!(
            group.keep.scanned.source_path,
            PathBuf::from("a-copy/game-a.bin")
        );
        assert_eq!(group.extra.len(), 1);
        assert_eq!(
            group.extra[0].scanned.source_path,
            PathBuf::from("z-copy/game-a.bin")
        );
    }
}

use crate::matcher::MatchReport;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixActionKind {
    /// The ROM is entirely missing from the scanned directory.
    Obtain,
    /// A file exists under the expected name but its hash does not match
    /// the DAT entry (corrupt dump, wrong revision, truncated download...).
    Replace,
}

impl std::fmt::Display for FixActionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FixActionKind::Obtain => write!(f, "OBTAIN"),
            FixActionKind::Replace => write!(f, "REPLACE"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FixAction {
    pub kind: FixActionKind,
    pub game_name: String,
    pub rom_name: String,
    pub expected_crc32: Option<String>,
    pub expected_size: u64,
    pub current_file: Option<PathBuf>,
    pub current_crc32: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FixReport {
    pub platform: String,
    pub completion_percent: f64,
    pub actions: Vec<FixAction>,
}

/// Turns a [`MatchReport`] into a precise, actionable list of what is needed
/// to complete the set: which ROMs to obtain (missing entirely) and which
/// files to replace (present but not matching the DAT).
pub fn build_fix_report(match_report: &MatchReport) -> FixReport {
    let mut actions: Vec<FixAction> = match_report
        .missing
        .iter()
        .map(|missing| FixAction {
            kind: FixActionKind::Obtain,
            game_name: missing.game_name.clone(),
            rom_name: missing.rom_name.clone(),
            expected_crc32: missing.expected_crc32.clone(),
            expected_size: missing.expected_size,
            current_file: None,
            current_crc32: None,
        })
        .collect();

    actions.extend(match_report.corrupt.iter().map(|rom_match| FixAction {
        kind: FixActionKind::Replace,
        game_name: rom_match.matched_game.clone().unwrap_or_default(),
        rom_name: rom_match.matched_rom.clone().unwrap_or_default(),
        expected_crc32: None,
        expected_size: 0,
        current_file: Some(rom_match.scanned.source_path.clone()),
        current_crc32: Some(rom_match.scanned.hashes.crc32.clone()),
    }));

    actions.sort_by(|a, b| {
        a.game_name
            .cmp(&b.game_name)
            .then_with(|| a.rom_name.cmp(&b.rom_name))
    });

    FixReport {
        platform: match_report.platform.clone(),
        completion_percent: match_report.completion_percent(),
        actions,
    }
}

impl FixReport {
    pub fn is_complete(&self) -> bool {
        self.actions.is_empty()
    }

    /// Human-readable, line-oriented report suitable for the terminal.
    pub fn to_text(&self) -> String {
        if self.actions.is_empty() {
            return format!(
                "{}: set is complete (100%). Nothing to fix.\n",
                self.platform
            );
        }

        let mut out = format!(
            "{}: {:.1}% complete — {} action(s) needed\n",
            self.platform,
            self.completion_percent,
            self.actions.len()
        );
        for action in &self.actions {
            match action.kind {
                FixActionKind::Obtain => {
                    out.push_str(&format!(
                        "  [OBTAIN]  {} — {} (expected CRC32 {}, {} bytes)\n",
                        action.game_name,
                        action.rom_name,
                        action.expected_crc32.as_deref().unwrap_or("?"),
                        action.expected_size,
                    ));
                }
                FixActionKind::Replace => {
                    out.push_str(&format!(
                        "  [REPLACE] {} — {} (found CRC32 {} at {})\n",
                        action.game_name,
                        action.rom_name,
                        action.current_crc32.as_deref().unwrap_or("?"),
                        action
                            .current_file
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default(),
                    ));
                }
            }
        }
        out
    }

    fn csv_field(value: &str) -> String {
        if value.contains(',') || value.contains('"') || value.contains('\n') {
            format!("\"{}\"", value.replace('"', "\"\""))
        } else {
            value.to_string()
        }
    }

    pub fn to_csv(&self) -> String {
        let mut out = String::from(
            "action,game,rom,expected_crc32,expected_size,current_file,current_crc32\n",
        );
        for action in &self.actions {
            out.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                action.kind,
                Self::csv_field(&action.game_name),
                Self::csv_field(&action.rom_name),
                Self::csv_field(action.expected_crc32.as_deref().unwrap_or("")),
                action.expected_size,
                Self::csv_field(
                    &action
                        .current_file
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default()
                ),
                Self::csv_field(action.current_crc32.as_deref().unwrap_or("")),
            ));
        }
        out
    }
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
  <game name="Game A">
    <rom name="game-a.bin" size="4" crc="b1f7f5a0"/>
  </game>
  <game name="Game B">
    <rom name="game-b.bin" size="4" crc="ffffffff"/>
  </game>
</datafile>"#;

    fn scanned(file_name: &str, crc32: &str) -> ScannedRom {
        ScannedRom {
            platform_hint: "Test".into(),
            source_path: PathBuf::from(file_name),
            archive_entry: None,
            file_name: file_name.into(),
            hashes: FileHashes {
                size: 4,
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
    fn lists_missing_and_corrupt_as_actions() {
        let gameset = parse_dat_str(SAMPLE, "Test").unwrap();
        let scan = vec![scanned("game-a.bin", "deadbeef")];
        let match_report = match_scan(&gameset, &scan);
        let fix = build_fix_report(&match_report);

        assert_eq!(fix.actions.len(), 2);
        assert!(fix
            .actions
            .iter()
            .any(|a| a.kind == FixActionKind::Replace && a.rom_name == "game-a.bin"));
        assert!(fix
            .actions
            .iter()
            .any(|a| a.kind == FixActionKind::Obtain && a.rom_name == "game-b.bin"));
    }

    #[test]
    fn reports_complete_when_nothing_to_fix() {
        let gameset = parse_dat_str(SAMPLE, "Test").unwrap();
        let scan = vec![
            scanned("game-a.bin", "b1f7f5a0"),
            scanned("game-b.bin", "ffffffff"),
        ];
        let match_report = match_scan(&gameset, &scan);
        let fix = build_fix_report(&match_report);

        assert!(fix.is_complete());
        assert!(fix.to_text().contains("complete"));
    }
}

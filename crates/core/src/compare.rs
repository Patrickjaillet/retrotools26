use crate::scan::ScannedRom;

#[derive(Debug, Clone, Default)]
pub struct SetComparison {
    /// Present in "after" but not in "before" (by content hash).
    pub added: Vec<ScannedRom>,
    /// Present in "before" but not in "after" (by content hash).
    pub removed: Vec<ScannedRom>,
    /// Same file name in both, but different content (before, after).
    pub changed: Vec<(ScannedRom, ScannedRom)>,
    pub unchanged_count: usize,
}

impl SetComparison {
    pub fn to_text(&self) -> String {
        let mut out = format!(
            "{} added, {} removed, {} changed, {} unchanged\n",
            self.added.len(),
            self.removed.len(),
            self.changed.len(),
            self.unchanged_count
        );
        for rom in &self.added {
            out.push_str(&format!("  + {}\n", rom.source_path.display()));
        }
        for rom in &self.removed {
            out.push_str(&format!("  - {}\n", rom.source_path.display()));
        }
        for (before, after) in &self.changed {
            out.push_str(&format!(
                "  ~ {} (crc32 {} -> {})\n",
                after.source_path.display(),
                before.hashes.crc32,
                after.hashes.crc32
            ));
        }
        out
    }
}

/// Compares two ROM scans (e.g. the same folder before/after a build, or two
/// different folders) by content hash. Files whose CRC32 only moved between
/// the two scans are not reported as changes; a file is "changed" when the
/// same file name is present on both sides with different content.
pub fn compare_scans(before: &[ScannedRom], after: &[ScannedRom]) -> SetComparison {
    use std::collections::HashSet;

    let before_crcs: HashSet<&str> = before.iter().map(|r| r.hashes.crc32.as_str()).collect();
    let after_crcs: HashSet<&str> = after.iter().map(|r| r.hashes.crc32.as_str()).collect();

    let mut candidate_added: Vec<&ScannedRom> = after
        .iter()
        .filter(|r| !before_crcs.contains(r.hashes.crc32.as_str()))
        .collect();
    let mut candidate_removed: Vec<&ScannedRom> = before
        .iter()
        .filter(|r| !after_crcs.contains(r.hashes.crc32.as_str()))
        .collect();

    let unchanged_count = after.len() - candidate_added.len();

    let mut changed = Vec::new();
    let mut added = Vec::new();
    for entry in candidate_added.drain(..) {
        if let Some(pos) = candidate_removed
            .iter()
            .position(|r| r.file_name == entry.file_name)
        {
            let before_entry = candidate_removed.remove(pos);
            changed.push((before_entry.clone(), entry.clone()));
        } else {
            added.push(entry.clone());
        }
    }
    let removed: Vec<ScannedRom> = candidate_removed.into_iter().cloned().collect();

    SetComparison {
        added,
        removed,
        changed,
        unchanged_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::FileHashes;
    use crate::header::RomHeaderKind;
    use std::path::PathBuf;

    fn rom(file_name: &str, crc32: &str) -> ScannedRom {
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
    fn detects_added_and_removed() {
        let before = vec![rom("a.bin", "111"), rom("b.bin", "222")];
        let after = vec![rom("a.bin", "111"), rom("c.bin", "333")];
        let diff = compare_scans(&before, &after);
        assert_eq!(diff.unchanged_count, 1);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].file_name, "c.bin");
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].file_name, "b.bin");
        assert!(diff.changed.is_empty());
    }

    #[test]
    fn detects_changed_when_name_matches_but_hash_differs() {
        let before = vec![rom("a.bin", "111")];
        let after = vec![rom("a.bin", "999")];
        let diff = compare_scans(&before, &after);
        assert_eq!(diff.changed.len(), 1);
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
    }
}

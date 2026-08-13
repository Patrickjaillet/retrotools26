//! Non-regression test at No-Intro-like scale.
//!
//! The roadmap asks for regression tests against real No-Intro/Redump DAT
//! samples. Those are copyrighted third-party downloads this environment
//! has no network access to fetch and no license to redistribute inside the
//! repo, so this is **not** that — it's a programmatically generated DAT
//! (~600 games, ~150 clone families) that follows the same naming
//! conventions real No-Intro DATs use (`Name (Region)`, `(Rev A)`,
//! `(Beta 1)`, `(Proto)`, parent/clone via matching canonical titles), at a
//! scale big enough to catch the kind of bug a handful of hand-written
//! 3-game fixtures wouldn't: an off-by-one in clone grouping, a rule that
//! accidentally keeps two releases of the same family, quadratic behavior
//! that would time out on a real ~3000-entry No-Intro DAT.

use retrotools_core::{dat, preview_selection, RulePriority};
use std::fmt::Write as _;

const REGIONS: &[&str] = &["Europe", "USA", "Japan", "World"];
const FAMILY_COUNT: usize = 150;

fn crc32_hex(data: &[u8]) -> String {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    format!("{:08x}", !crc)
}

/// Builds a synthetic No-Intro-style DAT: each of `FAMILY_COUNT` families
/// gets a release in every region, plus a revision and a beta for good
/// measure, so 1G1R selection has real work to do (region priority,
/// revision scoring, beta exclusion) at scale.
fn build_synthetic_dat() -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\"?>\n<datafile>\n  <header><name>Synthetic No-Intro Style</name><version>1</version></header>\n",
    );
    for family in 0..FAMILY_COUNT {
        let title = format!("Synthetic Game {family:04}");
        for region in REGIONS {
            let name = format!("{title} ({region})");
            let rom_name = format!("{name}.bin");
            let crc = crc32_hex(rom_name.as_bytes());
            writeln!(
                xml,
                "  <game name=\"{name}\"><rom name=\"{rom_name}\" size=\"1\" crc=\"{crc}\"/></game>"
            )
            .unwrap();
        }
        // A revision of the region we expect to win (Europe), which should
        // outrank the plain Europe release once selection runs.
        let rev_name = format!("{title} (Europe) (Rev A)");
        let rev_rom = format!("{rev_name}.bin");
        writeln!(
            xml,
            "  <game name=\"{rev_name}\"><rom name=\"{rev_rom}\" size=\"1\" crc=\"{}\"/></game>",
            crc32_hex(rev_rom.as_bytes())
        )
        .unwrap();
        // A beta, which the default rules exclude outright.
        let beta_name = format!("{title} (Europe) (Beta 1)");
        let beta_rom = format!("{beta_name}.bin");
        writeln!(
            xml,
            "  <game name=\"{beta_name}\"><rom name=\"{beta_rom}\" size=\"1\" crc=\"{}\"/></game>",
            crc32_hex(beta_rom.as_bytes())
        )
        .unwrap();
    }
    xml.push_str("</datafile>\n");
    xml
}

#[test]
fn parses_and_selects_a_no_intro_scale_dat_without_dropping_or_duplicating_families() {
    let xml = build_synthetic_dat();
    let expected_game_count = FAMILY_COUNT * (REGIONS.len() + 2); // +revision +beta

    let started = std::time::Instant::now();
    let gameset = dat::parse_dat_str(&xml, "Synthetic").unwrap();
    let parse_elapsed = started.elapsed();
    assert_eq!(gameset.games.len(), expected_game_count);

    let rules = RulePriority::default(); // Europe > USA > Japan > World; excludes Beta
    let selection_started = std::time::Instant::now();
    let preview = preview_selection(&gameset.games, &rules);
    let selection_elapsed = selection_started.elapsed();

    // Exactly one kept release per family: no family lost entirely, none
    // duplicated into two kept releases.
    assert_eq!(
        preview.kept.len(),
        FAMILY_COUNT,
        "expected exactly one kept release per family"
    );

    for family in 0..FAMILY_COUNT {
        let title = format!("Synthetic Game {family:04}");
        let kept_for_family: Vec<&str> = preview
            .kept
            .iter()
            .filter(|g| g.name.starts_with(&title))
            .map(|g| g.name.as_str())
            .collect();
        assert_eq!(
            kept_for_family.len(),
            1,
            "family '{title}' should have exactly one kept release, got {kept_for_family:?}"
        );
        // The revision should win over the plain Europe release.
        assert_eq!(kept_for_family[0], format!("{title} (Europe) (Rev A)"));
    }

    // No beta ever survives selection.
    assert!(preview.kept.iter().all(|g| !g.name.contains("Beta")));

    // Not a strict performance gate (this environment's CPU isn't a fixed
    // baseline), just a sanity ceiling so a real algorithmic regression
    // (e.g. accidental O(n^2) clone grouping) fails loudly instead of the
    // test just quietly taking minutes.
    assert!(
        parse_elapsed.as_secs() < 5,
        "parsing {expected_game_count} games took {parse_elapsed:?}, expected well under 5s"
    );
    assert!(
        selection_elapsed.as_secs() < 5,
        "selecting over {expected_game_count} games took {selection_elapsed:?}, expected well under 5s"
    );
}

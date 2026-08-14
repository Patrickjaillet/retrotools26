//! End-to-end integration tests for `retrotools_core`, exercised only
//! through its public API (this is a `tests/` binary, so it can't reach
//! private items) — the same sequence the CLI's `build1g1r` command runs:
//! scan -> match -> preview 1G1R selection -> filter -> plan -> execute.
//!
//! Every existing test elsewhere in the crate covers one module in
//! isolation (scan alone, matcher alone, rules alone, fileops alone); none
//! of them proves the modules actually compose correctly together against
//! real files on disk, which is what this file is for.

use retrotools_core::{
    dat, execute_build, match_scan, plan_build, preview_selection, scan, BuildOptions, OrganizeBy,
    RulePriority, ScanOptions, TransferMode, UndoLog,
};
use std::collections::HashSet;
use std::path::PathBuf;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rt26-integration-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const DAT: &str = r#"<?xml version="1.0"?>
<datafile>
  <header><name>Test System</name><version>1</version></header>
  <game name="Test Game (Europe)">
    <rom name="Test Game (Europe).bin" size="8" crc="214d601a"/>
  </game>
  <game name="Test Game (USA)">
    <rom name="Test Game (USA).bin" size="7" crc="17dcf4e8"/>
  </game>
  <game name="Other Game (USA)">
    <rom name="Other Game (USA).bin" size="5" crc="d9583520"/>
  </game>
</datafile>"#;

/// Full pipeline: scan a real ROM folder, match against a real DAT, run the
/// 1G1R selection, and materialize the result on disk — then check both the
/// filesystem state and the undo log agree with what was actually done.
#[test]
fn scan_match_select_and_build_pipeline_produces_the_expected_files() {
    let root = temp_dir("pipeline-root");
    std::fs::write(root.join("Test Game (Europe).bin"), b"europe-a").unwrap();
    std::fs::write(root.join("Test Game (USA).bin"), b"usa-bcd").unwrap();
    std::fs::write(root.join("Other Game (USA).bin"), b"other").unwrap();
    std::fs::write(root.join("junk.txt"), b"not a rom the dat knows about").unwrap();

    let gameset = dat::parse_dat_str(DAT, "Test System").unwrap();

    let scan_options = ScanOptions {
        roots: vec![root.clone()],
        recursive: true,
        scan_inside_archives: true,
    };
    let outcome = scan(&scan_options, None, None).unwrap();
    assert_eq!(
        outcome.roms.len(),
        4,
        "all 4 files on disk should be hashed"
    );
    assert!(outcome.errors.is_empty());

    let match_report = match_scan(&gameset, &outcome.roms);
    assert_eq!(
        match_report.matched.len(),
        3,
        "3 files match a DAT rom by CRC32"
    );
    assert_eq!(
        match_report.unknown.len(),
        1,
        "junk.txt matches nothing in the DAT"
    );
    assert!(match_report.missing.is_empty());
    assert!(match_report.corrupt.is_empty());

    // Default rules prefer Europe over USA, so the 1G1R selection should
    // drop "Test Game (USA)" and keep the other two games.
    let rules = RulePriority::default();
    let preview = preview_selection(&gameset.games, &rules);
    let kept_names: HashSet<&str> = preview.kept.iter().map(|g| g.name.as_str()).collect();
    assert!(kept_names.contains("Test Game (Europe)"));
    assert!(!kept_names.contains("Test Game (USA)"));
    assert!(kept_names.contains("Other Game (USA)"));

    let mut selected_report = match_report.clone();
    selected_report.matched.retain(|m| {
        m.matched_game
            .as_deref()
            .is_some_and(|g| kept_names.contains(g))
    });
    assert_eq!(selected_report.matched.len(), 2);

    let dest_root = temp_dir("pipeline-dest");
    let options = BuildOptions {
        destination_root: dest_root.clone(),
        mode: TransferMode::Copy,
        organize: OrganizeBy::ByPlatform,
        rename_to_dat_name: true,
    };
    let plans = plan_build(&gameset, &selected_report, &options);
    assert_eq!(plans.len(), 2);

    let undo_log = UndoLog::open_in_memory().unwrap();
    let (outcomes, batch_id) = execute_build(
        &plans,
        false,
        true,
        Some(&undo_log),
        "integration test build",
    )
    .unwrap();
    let batch_id =
        batch_id.expect("a real (non-dry-run) build with an undo log returns a batch id");

    for outcome in &outcomes {
        assert!(outcome.performed, "transfer failed: {:?}", outcome.error);
        assert_eq!(
            outcome.verified,
            Some(true),
            "post-copy hash verification should pass"
        );
    }

    let platform_dir = dest_root.join(gameset.platform);
    let europe_dest = platform_dir.join("Test Game (Europe).bin");
    let other_dest = platform_dir.join("Other Game (USA).bin");
    let usa_dest = platform_dir.join("Test Game (USA).bin");

    assert!(
        europe_dest.is_file(),
        "the kept Europe release should be on disk"
    );
    assert!(other_dest.is_file(), "the unrelated game should be on disk");
    assert!(
        !usa_dest.exists(),
        "the dropped USA duplicate must never be written"
    );
    assert_eq!(std::fs::read(&europe_dest).unwrap(), b"europe-a");
    assert_eq!(std::fs::read(&other_dest).unwrap(), b"other");

    // The undo log should let us cleanly reverse exactly what was built.
    let undo_outcome = undo_log.undo_batch(&batch_id).unwrap();
    assert_eq!(undo_outcome.reverted, 2);
    assert!(!europe_dest.exists());
    assert!(!other_dest.exists());
    // Copy mode never touches the source.
    assert!(root.join("Test Game (Europe).bin").exists());
    assert!(root.join("Other Game (USA).bin").exists());

    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&dest_root).ok();
}

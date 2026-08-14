pub mod archive;
pub mod cache;
pub mod compare;
pub mod convert;
pub mod dat;
pub mod dat_library;
pub mod dat_update;
pub mod external_tools;
pub mod fileops;
pub mod fix;
pub mod hash;
pub mod header;
pub mod matcher;
pub mod model;
pub mod profiles;
pub mod rebuild;
pub mod report;
pub mod rules;
pub mod scan;
pub mod undo;
pub mod watcher;

pub use cache::{DatCache, ScanCache};
pub use compare::{compare_scans, SetComparison};
pub use convert::{
    convert_from_chd, convert_from_cso, convert_from_rvz, convert_to_chd, convert_to_cso,
    convert_to_rvz,
};
pub use dat_library::{platforms_missing_dat, DatEntry, DatLibrary};
pub use dat_update::{check_for_update, download_dat, DatSource, DatUpdateReport};
pub use fileops::{
    execute_build, plan_build, safe_delete, BuildOptions, OrganizeBy, PlannedTransfer,
    TransferMode, TransferOutcome,
};
pub use fix::{build_fix_report, FixAction, FixActionKind, FixReport};
pub use matcher::{
    find_duplicate_matches, match_scan, DuplicateGroup, MatchReport, MissingRom, RomMatch,
    RomStatus,
};
pub use model::{DatHeader, DatType, Game, GameSet, Language, Region, RomFile};
pub use profiles::{built_in_presets, ProfileStore, RuleProfile};
pub use rebuild::{rebuild_to_archives, RebuildFormat, RebuildOutcome};
pub use report::ScanReport;
pub use rules::{
    preview_selection, select_one_game_one_rom, RulePriority, SelectionExplanation, SelectionResult,
};
pub use scan::{scan, ScanOptions, ScanOutcome, ScanProgress, ScannedRom};
pub use undo::{BatchSummary, UndoLog, UndoOutcome};
pub use watcher::FolderWatcher;

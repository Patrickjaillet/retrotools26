# Changelog

All notable changes to Retro Tools 2026 are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Full-pipeline integration test (`crates/core/tests/integration_pipeline.rs`): scan → match → 1G1R preview/selection → filter → plan → execute against real files on disk, plus post-transfer verification and undo, using only the public API — the previous test suite only ever covered one module at a time
- Large-scale synthetic-DAT non-regression test (`crates/core/tests/synthetic_no_intro_regression.rs`): a generated ~750-game, ~150-family DAT modeled on real No-Intro naming conventions (region/revision/beta tags), checked for zero lost or duplicated families after 1G1R selection, with a runtime ceiling to catch algorithmic regressions; real No-Intro/Redump sample DATs aren't bundled (third-party copyright, no network access here to fetch them)
- `criterion` benchmarks for hashing (`crates/core/benches/hashing.rs`, 1/8/32 MB buffers) and DAT parsing (`crates/core/benches/dat_parsing.rs`, 500/3000-game synthetic DATs), runnable via `cargo bench -p retrotools-core`
- Headless automated UI smoke tests (`crates/ui/src/main.rs::ui_smoke_tests`): drive every tab's `show()` against a real `egui::Context` with no window/event loop, across several frames, with and without an imported DAT, both in Expert and Wizard 1G1R modes; `egui_kittest` isn't published for the pinned egui/eframe 0.28, so this is a small hand-rolled harness instead
- Security review of archive extraction and file-build path construction: confirmed no zip-slip vector (entries are always streamed to a caller-supplied writer, never used to build a filesystem path) and added adversarial tests proving a malicious DAT-supplied game/rom name (`../../../../etc/evil`, backslash traversal) can never escape the configured destination root
- Test proving a build batch degrades gracefully when one transfer fails (destination blocked by a pre-existing file, the same failure class as a full disk or a permissions error): the failing item is reported per-item, the rest of the batch still completes
- ROM/ISO format converter (`retrotools_core::convert`): `convert_to_chd`/`convert_from_chd`, reusing the already-bundled `chdman.exe` (the same tool used for CHD extraction since Phase 2); new CLI `convert to-chd <source> <dest.chd>` and `convert from-chd <source.chd> <dest_dir>`; verified with a real round trip (raw image → CHD → extracted back → byte-for-byte identical). CSO (PSP) and RVZ (Dolphin) are still out of scope — each needs its own dedicated codec (`maxcso`, `DolphinTool`) that isn't bundled and isn't a byproduct of any other feature
- Missing-DAT detection and assisted download (`retrotools_core::dat_library::platforms_missing_dat`): flags ROM subfolders with no matching DAT imported yet; new CLI `dat detect-missing <roms_root> <dat_dir> [--assist]` and a new "Missing DATs" section in the UI's Platforms tab (`Detect missing DATs...` button, then a per-platform `Download & import` button when a same-named DAT source is already tracked) — reuses the existing tracked-source infrastructure rather than attempting real DAT discovery, since no unauthenticated No-Intro/Redump/TOSEC API exists for that; verified end-to-end (CLI and UI) against a real local HTTP server
- Games grid view (`views/games.rs::show_grid`), selectable alongside the existing list view via a List/Grid toggle; card-based layout with a status-colored placeholder tile (no real artwork yet — no scraper until Phase 7)
- Wizard mode for the 1G1R tab (`views/onegameonerom.rs::wizard`, driven by the new `AppState::wizard_step`/`WizardStep`): a step-by-step Platform → Scan → Rules → Preview → Build flow with a clickable breadcrumb and Back/Next navigation, reusing the same section-rendering functions as the existing single-page layout
- Expert mode toggle in the 1G1R tab (`AppState::expert_mode`, on by default): switches between the wizard and the original all-on-one-page layout
- Command palette (`Ctrl+Shift+P` or the new 🔍 button): fuzzy-filtered command list with arrow-key navigation and Enter/click execution — jump to any tab, import a DAT, toggle expert mode, toggle light/dark theme
- Additional keyboard shortcuts: `Alt+1` through `Alt+7` jump directly to each tab
- Minimal i18n layer (`crates/ui/src/i18n.rs`): `Language`/`Key`/`t()`, with a language selector in Settings that switches the UI immediately; covers tabs, the dashboard and a handful of key buttons/labels rather than every string, matching the roadmap's "structure ready" goal
- Adjustable UI scale for accessibility (`AppConfig::ui_scale`, a Settings slider applied via `egui::Context::set_pixels_per_point` and persisted across restarts)
- Light fade-in transition when switching tabs (`egui::Context::animate_bool_with_time` in `app.rs::central_content`), as lightweight visual feedback without pulling in a tweening library
- Duplicate ROM detection (`retrotools-core::matcher::find_duplicate_matches`): identifies extra copies of a ROM that already matched a DAT entry elsewhere (as opposed to `unknown`, which is content the DAT doesn't recognize at all) and picks a deterministic file to keep; the CLI `clean` command gained `--duplicates` (opt-in, off by default to stay backward-compatible) and `--no-unknown`, trashing both categories through the same undo-able safe-delete path with the reason labeled in its output
- PDF export for scan reports (`ScanReport::to_pdf`, via `printpdf`'s HTML-to-PDF renderer, reusing the existing `to_html` layout instead of a separate PDF-specific one); new CLI `scan --pdf <path>` option
- RAR and CHD archive support (`retrotools-core::external_tools`, extending `archive.rs`): shells out to bundled third-party CLI tools under `resources/` (`UnRAR.exe` for read-only RAR extraction, `chdman.exe` for CHD disk images — see `docs/COMPILATION.md` for licensing notes and packaging instructions) since neither format has a maintained pure-Rust decoder; tool resolution checks an env var override, a `resources/` folder near the executable, then `PATH`; verified with a real create→scan→hash→extract→compare round trip against a CHD built by `chdman` itself
- Real-time scan throughput (files/s, MB/s) in the 1G1R tab's progress indicator (`AppState::scan_speed`)
- DAT auto-update from a tracked download URL (`retrotools-core::dat_update`): register a name + direct DAT/ZIP URL once, then re-fetch it on demand and compare its version against the local `DatCache` to report whether anything actually changed, without ever re-importing an unchanged DAT; new CLI commands `dat source-add`/`dat source-list`/`dat source-remove`/`dat update`/`dat update-all`, and a matching "DAT update sources" section in the Settings tab (background thread, doesn't block the UI)
- Internal plugin architecture (`retrotools-plugin-api`): a `Plugin` trait, `PluginContext` and `PluginRegistry` let optional modules extend the app without modifying `retrotools-core` or `retrotools-ui`; plugins are statically-linked crates registered at startup rather than dynamically loaded `.dll`s (see `docs/PLUGIN_DEV.md` for the rationale and a step-by-step guide)
- Playlist Generator plugin (`retrotools-plugin-playlists`): generates RetroArch `.lpl`, LaunchBox XML and ES-DE `gamelist.xml` playlists from the current 1G1R selection
- BIOS Manager plugin (`retrotools-plugin-bios`): verifies a folder of BIOS files against a BIOS DAT (the same Logiqx/XML format No-Intro publishes for BIOS packs) by reusing the core scan/match engine, so no BIOS checksum is ever hardcoded
- New CLI commands `plugin list` and `plugin run <id> <dat> --output <dir> [--source <dir>] [--profile <name>]`
- New **Plugins** tab in the UI, listing every registered plugin with a `Run` button; validated end-to-end (source/output folder pickers, real files written to disk, live "last run" status per plugin)
- "Fix report" (`retrotools-core::fix`): turns a scan/match result into a precise, actionable list of what's needed to complete a set — ROMs to obtain (missing) and files to replace (present but not matching the DAT) — exportable as text/CSV, shown in the 1G1R tab and via the new CLI `fix` command
- Set comparator (`retrotools-core::compare`): diffs two ROM scans by content hash (added/removed/changed, with rename detection via matching file names), exposed via the new CLI `compare` command
- Real-time folder watcher (`retrotools-core::watcher`, backed by `notify`) and periodic auto-rescan, both toggleable from the 1G1R tab — a folder change now triggers an automatic re-scan (3s debounce) without any user action
- Multi-platform library overview via the new CLI `status` command: scans every platform subfolder of a ROMs root against its matching DAT and prints a completion table for the whole collection
- Fixed a real contrast bug found during visual QA: `theme.rs`'s selection stroke color was identical to the selection background, making selected tab/row text invisible; it's now computed for contrast against the accent color
- Full desktop UI wired to the core engine (`crates/ui`): a new `AppState` (`crates/ui/src/state.rs`) centralizes the imported DAT library, per-platform scan/match results, 1G1R rules and background scan/build jobs (dedicated threads + `mpsc` channels so the UI never blocks)
- Platforms tab: import DAT files (button or drag & drop), list imported platforms with type/version/game count, select/remove
- Games tab: searchable, filterable (region/language/status) and sortable (name/region/status/size) game list with a details panel (regions, languages, tags, ROM files, scan status, 1G1R kept/removed state)
- 1G1R tab: guided flow — scan a ROM folder with a live progress indicator, edit region/language priority and exclusion rules, preview the selection (kept/removed diff with per-choice explanations), load/save rule profiles, and build the set (copy/move/hardlink/symlink, dry-run, rename-to-DAT-name, folder organization)
- Settings tab now edits and persists the real `AppConfig` (theme, accent color, ROM/DAT directories, log level, update check)
- Dashboard shows real statistics (platforms, DAT files, games tracked, completion, per-platform breakdown) instead of placeholders
- Toast notifications wired to real actions (DAT import, scan, build, errors) instead of being unused
- `Ctrl+O` keyboard shortcut to import a DAT; drag & drop of `.dat`/`.xml`/`.zip` files anywhere in the window
- File-operation engine for materializing a 1G1R set (`retrotools-core::fileops`): copy, move, hardlink or symlink the selected ROMs, with automatic fallback to copy when the source lives inside an archive; destination filenames can follow the DAT's canonical ROM name and be organized into platform/region subfolders
- Dry-run mode and post-transfer verification (re-hashes the destination and compares it to the source) for every build operation
- Operation history with undo (`retrotools-core::undo::UndoLog`, SQLite-backed): every build/clean batch can be listed and reversed via `undo list`/`undo apply`
- Safe delete: the `clean` command moves files unrecognized by the DAT into a local trash folder instead of deleting them outright, reversible through the same undo log
- Archive rebuild (`retrotools-core::rebuild`): writes one ZIP per game from the matched ROMs, streaming directly from the original file or archive entry (no full extraction to disk)
- CLI commands `build1g1r`, `rebuild1g1r`, `clean`, and `undo list`/`undo apply`
- 1G1R selection engine (`retrotools-core::rules`): configurable region/language priority, per-category exclusion filters (Beta/Proto/Demo/Kiosk/Promo/Unlicensed/Pirate/Bad Dump), "prefer parent over clone" tie-breaking, and revision-tag scoring (Rev/v1.1/lettered revisions)
- Multi-disc/multi-file aware grouping: releases are grouped by canonical title (parentheses stripped) rather than by `cloneof`, since many real-world DATs leave clone metadata unset or inconsistent for discs beyond the first — every disc of the winning release is kept together
- Selection preview with before/after diff and a human-readable explanation per choice (`rules::preview_selection`, `SelectionResult`, `SelectionExplanation`)
- Reusable 1G1R rule profiles saved as JSON on disk, plus built-in presets "Standard Europe", "Standard USA" and "Complete - No Filter" (`retrotools-core::profiles`)
- New Game tag fields (`is_sample`, `is_kiosk`, `is_promo`, `is_pirate`, `is_alt`) parsed from DAT ROM names, extending the existing Beta/Proto/Demo/Unlicensed/Bad Dump detection
- CLI commands `select1g1r` and `profile list`/`profile show`
- Recursive, multi-root ROM directory scanner (`retrotools-core::scan`), parallelized with `rayon`
- Streaming CRC32/MD5/SHA1/SHA256 hashing in a single pass (`retrotools-core::hash`), including a second "headerless" hash when a known ROM container header is detected
- ROM header detection for iNES, Lynx and Atari 7800 (`retrotools-core::header`)
- Archive scanning without full extraction (`retrotools-core::archive`): ZIP, 7Z and TAR entries are streamed directly into the hasher; RAR and CHD are detected but not yet extractable
- ROM ↔ DAT matching (`retrotools-core::matcher`): exact hash matching with a filename-based fallback that flags corrupted/renamed dumps, plus "unknown" (unneeded) and "missing" reporting and a completion percentage
- Scan reports exportable as CSV and HTML (`retrotools-core::report::ScanReport`)
- Incremental scan cache (`retrotools-core::cache::ScanCache`, SQLite): skips re-hashing files unchanged since the last scan
- CLI command `scan` to scan a directory, optionally match against a DAT, and write CSV/HTML reports
- Logiqx/XML DAT parser (`retrotools-core::dat`) supporting No-Intro, Redump, TOSEC and MAME datfiles (`<datafile>/<game>` and `<mame>/<machine>` layouts), with automatic DAT type detection and region/language/tag extraction from ROM names
- Support for DAT files delivered inside ZIP archives
- Parent/clone relationship handling (`cloneof`, `romof`) with `GameSet::find_clones_of`
- DAT integrity validation (required fields, CRC32 format, dangling clone references)
- Multi-DAT library management (`retrotools-core::dat_library::DatLibrary`) for importing and tracking several DAT files at once
- Local SQLite cache of parsed DAT files (`retrotools-core::cache::DatCache`, via `rusqlite`)
- CLI commands `dat import`, `dat import-dir` and `dat validate` for parsing and validating DAT files from the command line
- Cargo workspace scaffold with four crates: `retrotools-common`, `retrotools-core`, `retrotools-ui`, `retrotools-cli`
- Centralized SemVer versioning via the workspace `Cargo.toml`, with git commit hash and build date injected at compile time
- Unified application error type (`AppError`) with severity levels and user-facing messages
- User configuration system persisted as TOML under the platform config directory
- Structured logging via `tracing`, writing daily rotating log files
- Native desktop application shell built with egui/eframe: top navigation, light/dark/system theme with accent color, toast notification system
- "About" tab displaying copyright, license, contact email, website and repository links
- Placeholder data model (`Game`, `RomFile`, `Region`, `Language`, `GameSet`) and stub modules for DAT parsing, ROM scanning and 1G1R rule selection, scheduled for upcoming phases
- Minimal command-line interface (`retrotools-cli`) exposing version and configuration path commands
- Continuous integration workflow: format check, Clippy lint, Windows build and test
- Professional repository scaffolding: `LICENSE` (MIT), `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, issue and pull request templates, `.gitignore`, `.gitattributes`

## [0.1.0] - Unreleased

Initial project scaffold (Phase 0 — Technical Foundations). No end-user functionality yet.

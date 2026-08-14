# Changelog

All notable changes to Retro Tools 2026 are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.5] - 2026-08-14

### Fixed
- `platform_badge::draw` used `centered_and_justified`, which claims all *available* space in its parent `Ui` — harmless in a plain vertical layout, but it stretched the badge into a full-width colored bar wherever it sat inside an `egui::Grid` cell (Platforms tab) or next to a heading (Dashboard, Games), discovered while taking real screenshots for the README. Now allocates its exact size via the `Painter`, same approach as `shader_preview::draw`.

### Changed
- `README.md` rewritten as a pure end-user showcase (what the app does, not how to build it) with 4 real screenshots (Dashboard, Games, Platforms, 1G1R Builder), replacing the previous 2-screenshot version with build-instructions front and center.

## [0.1.4] - 2026-08-14

### Fixed
- CI was red on every commit since early in the project and had gone unnoticed because local checks never ran with the same strict flags: `cargo clippy --workspace --all-targets -- -D warnings` failed on two long-standing warnings (`ThemePreference`'s and `ToastManager`'s manual `Default` impls, both now `#[derive(Default)]`; `ToastManager::report` was genuinely unused code and removed), and `cargo fmt --all -- --check` failed across most of the codebase (now reformatted to rustfmt defaults, no behavior change)

## [0.1.3] - 2026-08-14

### Fixed
- Dashboard, Platforms, Plugins, Settings, Download and About tabs now scroll (each wrapped in its own `egui::ScrollArea::vertical()`) — on a small or restored-from-minimized window, content past the bottom of the visible area was previously unreachable with no scrollbar; the Games and 1G1R tabs already had their own internal scrolling and are unaffected

### Added
- New "Download" tab (`crates/ui/src/views/download.rs`): official/community links for everything this app works with but doesn't bundle — DAT sources (No-Intro, Redump, TOSEC), frontends/distributions (Batocera, Recalbox, Lakka, EmulationStation-DE), RetroArch cores and shaders, RetroAchievements account creation, and the third-party RVZ/CSO conversion tools; deliberately excludes ROM download sites, with a visible note explaining why — this app manages ROMs you already own, it doesn't point at copyright-infringing sources

## [0.1.2] - 2026-08-14

Adds the "Modules Futurs Retrogaming" batch (Phases 11-20): Batocera/
Recalbox/Lakka export, ScreenScraper metadata scraping, ES-DE smart
collections, save backup/restore, controller profile library, shader
overrides, a core-compatibility advisor, RVZ/CSO conversion, SD/USB card
imaging, and RetroAchievements integration — plus generated platform
badges and shader preset previews in the UI.

### Added
- Static shader preset previews (`crates/ui/src/shader_preview.rs`): a small synthetic reference image shown next to each shader association and in the preset picker in Settings, illustrating the preset's category (CRT/scanline/pixel-art upscaler) with scanline overlays, a vignette, or a blocky-vs-smooth split — not a real screenshot, since this project doesn't bundle third-party shader/game screenshots; the file-name classification is a pure, unit-tested function kept separate from the drawing code
- Procedurally generated platform badges (`crates/ui/src/platform_badge.rs`): a deterministic colored circle with 2-4 letter initials derived from the platform's name, shown in the Platforms, Dashboard and Games tabs — real console/company logos are trademarks and can't legally be bundled in this public repository under any license this project could choose, so this sidesteps that entirely while still giving every platform a distinct visual identity
- RetroAchievements integration (`retrotools-plugin-retroachievements`): syncs and locally caches known RetroAchievements-compatible ROM hashes per platform (via an editable platform → console-id table) using your own username/API key (encrypted, Settings), then cross-references the current 1G1R selection and reports any kept game with no known hash; a new opt-in tie-breaker, `RulePriority::prefer_retroachievements_compatible` (off by default, changes nothing unless enabled), lets 1G1R selection itself prefer an RA-compatible alternate when otherwise tied — both read the same on-disk hash cache without sharing Rust state; RetroAchievements compatibility now also shows in the Games tab's detail panel
- SD/USB card imaging (`retrotools-plugin-sdcard-imager`): checksum verification of a downloaded Batocera/Recalbox/Lakka base image (`sdcard verify`), removable-USB-disk detection (`sdcard list-devices`), and raw image writing (`sdcard write`) gated by a typed double confirmation of the exact device id (nothing is written if `--device`/`--confirm` don't match) — the destructive write path is deliberately plain CLI-invoked functions rather than the generic plugin contract, since `PluginContext` has no room for that confirmation; `sdcard-inject` (a normal plugin) then mirrors a staging folder (built 1G1R set plus any Export/Controllers/Shaders output) onto the freshly-imaged, already-mounted partition; a manual real-hardware validation procedure is documented in `docs/PLUGIN_DEV.md` since writing to a real device is intentionally not automated in tests
- RVZ (GameCube/Wii) and CSO (PSP) conversion: `convert to-rvz`/`from-rvz` (via a third-party `DolphinTool`) and `convert to-cso`/`from-cso` (via a third-party `maxcso`), symmetric to the existing CHD conversion commands; neither tool is bundled with this app (unlike UnRAR/chdman/7za) — `docs/COMPILATION.md` explains how to obtain and point the app at them (`RETROTOOLS_DOLPHINTOOL_PATH`/`RETROTOOLS_MAXCSO_PATH`, a `resources/` folder, or `PATH`), and each command fails with a clear message rather than silently if the tool isn't found
- Core compatibility advisor (`retrotools-plugin-core-advisor`): a local, user-imported JSON database mapping platform/game → recommended libretro core (with a confidence level, a free-text note, and a "known problematic" flag), edited from a new Settings section; `core-advisor-report` cross-references the games the last scan actually matched (via the new `PluginContext::match_report`, no re-scanning) against that database, writes a plain-text report, and generates a per-game `.opt` core-options override at RetroArch's exact expected path for any entry with specific options; the recommended core and its note now also show in the Games tab's detail panel
- RetroArch shader override generation (`retrotools-plugin-shaders`): a library of `.glslp`/`.slangp` presets (3 realistic starters — CRT geom, scanlines, scale2x — whose shader paths were verified against the real `libretro/slang-shaders` folder layout so they actually resolve on a standard RetroArch install, though nothing is copied from that repository itself, plus import of externally-supplied preset files), a saved shader → core / shader → game association list edited from a new Settings section, and `shaders-export` which writes RetroArch override files at the exact path RetroArch itself expects (`<core>/<core>.cfg` for a whole core, `<core>/<content dir>/<game>.cfg` for one game); every generated file is recorded in a manifest so `shaders-clean` can remove exactly what this tool produced without ever touching a hand-written override
- `PluginContext` gained a `match_report: Option<&MatchReport>` field so a plugin can cross-reference per-ROM scan status (matched/corrupt/unknown/missing) without re-scanning — needed by the upcoming core-compatibility and RetroAchievements modules; the UI now threads its live `MatchReport` through, the CLI passes `None` (it doesn't currently keep one around between commands)

## [0.1.0] - 2026-08-13

First release: DAT parsing (No-Intro/Redump/TOSEC/MAME), ROM scanning and
hashing (incl. ZIP/7Z/RAR/CHD), 1G1R selection with configurable
region/language rules, file-build operations with undo, a static plugin
system, a full CLI, an egui desktop UI, and Windows packaging
(installer + portable build).

### Added
- EmulationStation/ES-DE custom collections (`retrotools-plugin-playlists::CollectionsPlugin`, id `es-de-collections`): generates `collections/custom-by-region-<region>.cfg`/`custom-by-language-<language>.cfg` files (one ROM path per line, the real ES-DE format) from the current 1G1R selection; when a `gamelist.xml` written by the scraper plugin is present, also generates genre- and release-year-based collections by reading the metadata back out of it; regenerating a collection only ever adds lines, never removes one, so anything added by hand survives
- `retrotools-plugin-scraper` now also captures genre and release year from ScreenScraper (`GameMedia::genre`/`release_year`, written as `gamelist.xml`'s `<genre>`/`<releasedate>`) specifically to feed the new dynamic collections
- Metadata scraper for ScreenScraper.fr (`retrotools-plugin-scraper`): looks up games by the CRC32 already in the DAT (no rehashing), downloads box art/screenshots/videos/wheel logos into a size-limited local cache (oldest files purged first), and writes/updates a `gamelist.xml` with non-destructive per-game merging; a rate limiter enforces a minimum delay between requests and retries transient failures (429/5xx/timeout) with backoff, giving up immediately on permanent ones (bad credentials); refuses to run with a clear message until credentials are configured
- `retrotools_common::secrets`: real OS-backed credential encryption via Windows DPAPI (`CryptProtectData`/`CryptUnprotectData`), tied to the current Windows login rather than a locally-stored key; used to store ScreenScraper developer/account credentials (`AppConfig.screenscraper`, new Settings section) — nothing is ever bundled with the app, every credential is user-supplied
- Controller autoconfig export (`retrotools-plugin-controllers`): a plain folder of real RetroArch autoconfig `.cfg` files as an editable, user-extensible library (two realistic starter profiles — Xbox 360 XInput, generic DirectInput — auto-seeded on first use); `controllers-export` validates and copies every profile in the library to a target `autoconfig/` folder in one pass, skipping and clearly reporting any invalid ones; `parse_autoconfig`/`serialize_autoconfig`/`validate_autoconfig` round-trip the format faithfully. No live gamepad-input capture (would need a new dependency like `gilrs`) — documented as out of scope for this pass, not silently dropped
- Save backup/restore (`retrotools-plugin-saves`): `saves-backup` zips a RetroArch saves/states folder into a single timestamped archive (deflate, original modification times preserved); `saves-restore` restores one back, moving any file it would overwrite into the trash first via `fileops::safe_delete` — the restore shows up in the same `undo list`/`undo apply` history as builds and cleans, and is fully reversible; validated end-to-end through the real CLI (backup → simulate further play → restore → undo → original content recovered)
- Batocera/Recalbox/Lakka export (`retrotools-plugin-batocera-export`): copies an already-built 1G1R set into a `roms/<system>/` tree for any of the three distributions, registered as three plugins (`export-batocera`/`export-recalbox`/`export-lakka`); merges a `<system>` entry into `es_systems.cfg` for the two EmulationStation-based distros (Batocera, Recalbox) without disturbing entries from previous exports, and skips it entirely for Lakka (no EmulationStation front-end there); the platform → system-folder-name table is user-editable JSON with a sensible built-in default and a clearly-flagged fallback for unmapped platforms
- `PluginContext` gained a `dry_run` field (all existing plugins ignore it harmlessly): a plugin can now support a real preview mode, wired into both the CLI (`plugin run ... --dry-run`) and the UI (a checkbox in the Plugins tab)
- `retrotools_common::config::plugin_data_dir_path`: a shared per-plugin editable-data location, so future plugins needing their own config/cache don't each invent a different path scheme
- Replaced the two literal "Placeholder preview" mockups in `docs/screenshot1.png`/`screenshot2.png` with real captures of the running app (Dashboard and 1G1R Builder, with an actual DAT imported); corrected a stale claim in `docs/PLUGIN_DEV.md` that CHD conversion was unimplemented (it was added in Phase 7, `retrotools_core::convert`)
- Real GitHub-Releases-backed auto-updater (`retrotools_common::updater::GitHubReleaseSource`): checks the public Releases API (no auth token needed) and compares versions; wired into the UI (startup check when `check_updates_on_startup` is on and a repository is configured in Settings, with a toast when an update is available) and the CLI (`check-update [--repository owner/repo]`); no repository is guessed by default (`AppConfig::update_repository` starts `None`)
- Windows installer (`packaging/installer.iss`, Inno Setup 7): per-user install (no admin rights), bundles `resources/*.exe` and docs, Start Menu/desktop shortcuts; unsigned (no code-signing certificate available)
- Portable build (`packaging/make_portable.ps1` + `retrotools_common::config::is_portable_mode()`): dropping a `portable.txt` marker next to the exe switches every config/cache/log/DAT path to `<exe_dir>/data/` instead of the per-user profile
- Multi-resolution app icon (`crates/ui/assets/icon.ico`, 16/32/48/256), procedurally generated (no source logo existed), embedded into the exe (`crates/ui/build.rs` via `winresource`) and set as the runtime window icon
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

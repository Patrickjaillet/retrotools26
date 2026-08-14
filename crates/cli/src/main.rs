use clap::{Parser, Subcommand, ValueEnum};
use retrotools_common::{current_version, AppConfig};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, ValueEnum)]
enum ModeArg {
    Copy,
    Move,
    Hardlink,
    Symlink,
}

impl From<ModeArg> for retrotools_core::TransferMode {
    fn from(value: ModeArg) -> Self {
        match value {
            ModeArg::Copy => retrotools_core::TransferMode::Copy,
            ModeArg::Move => retrotools_core::TransferMode::Move,
            ModeArg::Hardlink => retrotools_core::TransferMode::HardLink,
            ModeArg::Symlink => retrotools_core::TransferMode::SymLink,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum OrganizeArg {
    Flat,
    Platform,
    PlatformRegion,
}

impl From<OrganizeArg> for retrotools_core::OrganizeBy {
    fn from(value: OrganizeArg) -> Self {
        match value {
            OrganizeArg::Flat => retrotools_core::OrganizeBy::Flat,
            OrganizeArg::Platform => retrotools_core::OrganizeBy::ByPlatform,
            OrganizeArg::PlatformRegion => retrotools_core::OrganizeBy::ByPlatformAndRegion,
        }
    }
}

#[derive(Parser)]
#[command(name = "retrotools-cli")]
#[command(about = "Retro Tools 2026 - 1G1R command line interface", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Version,
    ConfigPath,
    /// Check GitHub Releases for a newer version (uses the repository
    /// configured via `dat` Settings.../ `update_repository` in config.toml)
    CheckUpdate {
        /// Override the configured "owner/repo" for this check
        #[arg(long)]
        repository: Option<String>,
    },
    #[command(subcommand)]
    Dat(DatCommands),
    /// Scan a ROM directory, hash every file/archive entry found, and
    /// optionally match the results against a DAT file
    Scan {
        root: PathBuf,
        /// DAT file to match scanned ROMs against
        #[arg(long)]
        dat: Option<PathBuf>,
        /// Write a CSV report to this path
        #[arg(long)]
        csv: Option<PathBuf>,
        /// Write an HTML report to this path
        #[arg(long)]
        html: Option<PathBuf>,
        /// Write a PDF report to this path
        #[arg(long)]
        pdf: Option<PathBuf>,
        /// Do not recurse into subdirectories
        #[arg(long)]
        no_recursive: bool,
        /// Do not scan inside ZIP/7Z/TAR archives
        #[arg(long)]
        no_archives: bool,
        /// Disable the incremental hash cache
        #[arg(long)]
        no_cache: bool,
    },
    /// Compute the 1G1R selection for a DAT file and print which entries
    /// would be kept/removed, with an explanation for each choice
    Select1g1r {
        dat: PathBuf,
        /// Name of a built-in or saved rule profile (default: built-in defaults)
        #[arg(long)]
        profile: Option<String>,
        /// List every kept/removed game instead of just the summary counts
        #[arg(long)]
        verbose: bool,
    },
    #[command(subcommand)]
    Profile(ProfileCommands),
    /// Scan a ROM directory, match it against a DAT, apply the 1G1R
    /// selection, and copy/move/link the kept files into `dest`
    Build1g1r {
        root: PathBuf,
        dat: PathBuf,
        dest: PathBuf,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long, value_enum, default_value = "copy")]
        mode: ModeArg,
        #[arg(long, value_enum, default_value = "platform")]
        organize: OrganizeArg,
        /// Name destination files after the DAT's canonical ROM name instead
        /// of keeping the name found on disk
        #[arg(long)]
        rename_to_dat_name: bool,
        /// Compute the plan and print it without touching the filesystem
        #[arg(long)]
        dry_run: bool,
        /// Skip re-hashing each destination file after the transfer
        #[arg(long)]
        no_verify: bool,
        #[arg(long)]
        no_cache: bool,
    },
    /// Same as `build1g1r`, but rebuilds each game into one ZIP archive per
    /// game instead of loose files
    Rebuild1g1r {
        root: PathBuf,
        dat: PathBuf,
        dest: PathBuf,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        no_cache: bool,
    },
    /// Scan a ROM directory and safely move every file that does not match
    /// the DAT ("unneeded") into a local trash folder
    Clean {
        root: PathBuf,
        dat: PathBuf,
        /// Also trash surplus copies of ROMs already matched elsewhere
        /// (identical content present at more than one path)
        #[arg(long)]
        duplicates: bool,
        /// Skip files unrecognized by the DAT (only makes sense together
        /// with --duplicates, otherwise there is nothing left to clean)
        #[arg(long)]
        no_unknown: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        no_cache: bool,
    },
    #[command(subcommand)]
    Undo(UndoCommands),
    /// Scan a ROM directory, match it against a DAT, and print exactly what
    /// is needed to complete the set (ROMs to obtain, files to replace)
    Fix {
        root: PathBuf,
        dat: PathBuf,
        /// Write the fix list as CSV to this path
        #[arg(long)]
        csv: Option<PathBuf>,
        #[arg(long)]
        no_cache: bool,
    },
    /// Scan two ROM directories and report which files were added, removed
    /// or changed between them (by content hash)
    Compare {
        before: PathBuf,
        after: PathBuf,
        #[arg(long)]
        no_recursive: bool,
        #[arg(long)]
        no_archives: bool,
    },
    /// Scan every subdirectory of a ROMs folder against the matching DAT
    /// found in a DAT library folder, and print a completion overview for
    /// the whole collection
    Status {
        roms_root: PathBuf,
        dat_dir: PathBuf,
        #[arg(long)]
        no_cache: bool,
    },
    #[command(subcommand)]
    Plugin(PluginCommands),
    #[command(subcommand)]
    Convert(ConvertCommands),
    #[command(subcommand)]
    Sdcard(SdcardCommands),
}

#[derive(Subcommand)]
enum SdcardCommands {
    /// List removable (USB) disks currently connected, with the exact id
    /// each one needs for `sdcard write`
    ListDevices,
    /// Verify a downloaded base image against an official checksum before
    /// writing it anywhere
    Verify {
        image: PathBuf,
        #[arg(long)]
        sha256: Option<String>,
        #[arg(long)]
        md5: Option<String>,
    },
    /// Write a base image to a removable device. DESTRUCTIVE AND
    /// IRREVERSIBLE: everything already on the device is erased. Requires
    /// typing the exact device id twice (--device and --confirm) as a
    /// safety gate against selecting the wrong disk
    Write {
        image: PathBuf,
        #[arg(long)]
        device: String,
        /// Must exactly match --device, or nothing is written
        #[arg(long)]
        confirm: String,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum ConvertCommands {
    /// Convert a raw disk image (.iso/.bin/.img) or CD sheet (.cue/.gdi/.toc/.nrg)
    /// into a CHD, via the bundled `chdman.exe`
    ToChd { source: PathBuf, dest: PathBuf },
    /// Extract a CHD back to a raw image (.bin, plus a .cue if it was a CD image),
    /// via the bundled `chdman.exe`
    FromChd { source: PathBuf, dest_dir: PathBuf },
    /// Convert a GameCube/Wii disc image (.iso/.gcm/.gcz) into RVZ, via a
    /// third-party `DolphinTool` (not bundled, see docs/COMPILATION.md)
    ToRvz { source: PathBuf, dest: PathBuf },
    /// Convert an RVZ back to a plain .iso, via a third-party `DolphinTool`
    /// (not bundled, see docs/COMPILATION.md)
    FromRvz { source: PathBuf, dest_dir: PathBuf },
    /// Compress a PSP disc image (.iso) into CSO, via a third-party `maxcso`
    /// (not bundled, see docs/COMPILATION.md)
    ToCso { source: PathBuf, dest: PathBuf },
    /// Decompress a CSO back to a plain .iso, via a third-party `maxcso`
    /// (not bundled, see docs/COMPILATION.md)
    FromCso { source: PathBuf, dest_dir: PathBuf },
}

#[derive(Subcommand)]
enum PluginCommands {
    /// List every plugin registered with this build
    List,
    /// Run a plugin by id
    Run {
        id: String,
        /// DAT the plugin should operate against (a ROM DAT for the
        /// playlist generator, a BIOS DAT for the BIOS manager)
        dat: PathBuf,
        /// Directory the plugin writes its output to
        #[arg(long)]
        output: PathBuf,
        /// Directory to scan/verify (required by some plugins, e.g. the BIOS manager)
        #[arg(long)]
        source: Option<PathBuf>,
        /// Restrict the plugin to a 1G1R selection computed with this rule profile
        /// instead of operating on every game in the DAT
        #[arg(long)]
        profile: Option<String>,
        /// Compute and report what the plugin would do, without writing anything
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum UndoCommands {
    /// List recorded build/clean batches that can be undone
    List,
    /// Reverse every action recorded under a batch id
    Apply { batch_id: String },
}

#[derive(Subcommand)]
enum ProfileCommands {
    /// List built-in presets and profiles saved on disk
    List,
    /// Print the rules contained in a profile (built-in or saved)
    Show { name: String },
}

#[derive(Subcommand)]
enum DatCommands {
    /// Parse a single DAT file (.dat/.xml/.zip) and print a summary
    Import { path: PathBuf },
    /// Import every DAT file found in a directory and print a summary per file
    ImportDir { dir: PathBuf },
    /// Validate a DAT file without importing it
    Validate { path: PathBuf },
    /// Track a direct DAT/ZIP download URL under a name, for later `dat update`
    SourceAdd { name: String, url: String },
    /// List tracked DAT update sources
    SourceList,
    /// Stop tracking a DAT update source
    SourceRemove { name: String },
    /// Download a tracked source's URL again and report whether the DAT version changed
    Update { name: String },
    /// Update every tracked DAT source
    UpdateAll,
    /// List ROM subfolders under `roms_root` that have no matching DAT imported from `dat_dir`
    DetectMissing {
        roms_root: PathBuf,
        dat_dir: PathBuf,
        /// For folders whose name matches a tracked DAT source, download and save it
        #[arg(long)]
        assist: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    let config = AppConfig::load().unwrap_or_default();
    let _guard = retrotools_common::logging::init_logging(&config.log_level);

    match cli.command {
        Some(Commands::Version) => {
            println!("Retro Tools 2026 {}", current_version());
        }
        Some(Commands::ConfigPath) => match retrotools_common::config::config_file_path() {
            Ok(path) => println!("{}", path.display()),
            Err(err) => eprintln!("error: {}", err),
        },
        Some(Commands::CheckUpdate { repository }) => run_check_update_command(repository),
        Some(Commands::Dat(dat_command)) => run_dat_command(dat_command),
        Some(Commands::Scan {
            root,
            dat,
            csv,
            html,
            pdf,
            no_recursive,
            no_archives,
            no_cache,
        }) => run_scan_command(
            root,
            dat,
            csv,
            html,
            pdf,
            no_recursive,
            no_archives,
            no_cache,
        ),
        Some(Commands::Select1g1r {
            dat,
            profile,
            verbose,
        }) => run_select1g1r_command(dat, profile, verbose),
        Some(Commands::Profile(profile_command)) => run_profile_command(profile_command),
        Some(Commands::Build1g1r {
            root,
            dat,
            dest,
            profile,
            mode,
            organize,
            rename_to_dat_name,
            dry_run,
            no_verify,
            no_cache,
        }) => run_build1g1r_command(
            root,
            dat,
            dest,
            profile,
            mode.into(),
            organize.into(),
            rename_to_dat_name,
            dry_run,
            !no_verify,
            no_cache,
        ),
        Some(Commands::Rebuild1g1r {
            root,
            dat,
            dest,
            profile,
            dry_run,
            no_cache,
        }) => run_rebuild1g1r_command(root, dat, dest, profile, dry_run, no_cache),
        Some(Commands::Clean {
            root,
            dat,
            duplicates,
            no_unknown,
            dry_run,
            no_cache,
        }) => run_clean_command(root, dat, duplicates, no_unknown, dry_run, no_cache),
        Some(Commands::Undo(undo_command)) => run_undo_command(undo_command),
        Some(Commands::Fix {
            root,
            dat,
            csv,
            no_cache,
        }) => run_fix_command(root, dat, csv, no_cache),
        Some(Commands::Compare {
            before,
            after,
            no_recursive,
            no_archives,
        }) => run_compare_command(before, after, no_recursive, no_archives),
        Some(Commands::Status {
            roms_root,
            dat_dir,
            no_cache,
        }) => run_status_command(roms_root, dat_dir, no_cache),
        Some(Commands::Plugin(plugin_command)) => run_plugin_command(plugin_command),
        Some(Commands::Convert(convert_command)) => run_convert_command(convert_command),
        Some(Commands::Sdcard(sdcard_command)) => run_sdcard_command(sdcard_command),
        None => {
            println!("Retro Tools 2026 {}", current_version());
            println!("Run with --help to see available commands.");
        }
    }
}

fn run_dat_command(command: DatCommands) {
    match command {
        DatCommands::Import { path } => match retrotools_core::dat::parse_dat_file(&path) {
            Ok(gameset) => print_summary(&path, &gameset),
            Err(err) => eprintln!("error: {}", err),
        },
        DatCommands::ImportDir { dir } => {
            let mut library = retrotools_core::DatLibrary::new();
            match library.import_dir(&dir) {
                Ok(results) => {
                    for result in results {
                        if let Err(err) = result {
                            eprintln!("error: {}", err);
                        }
                    }
                    for entry in library.entries() {
                        print_summary(&entry.source_path, &entry.gameset);
                    }
                }
                Err(err) => eprintln!("error: {}", err),
            }
        }
        DatCommands::Validate { path } => match retrotools_core::dat::parse_dat_file(&path) {
            Ok(_) => println!("{}: valid", path.display()),
            Err(err) => eprintln!("{}: invalid ({})", path.display(), err),
        },
        DatCommands::SourceAdd { name, url } => {
            let mut config = AppConfig::load().unwrap_or_default();
            config.dat_sources.retain(|s| s.name != name);
            config
                .dat_sources
                .push(retrotools_common::config::DatSourceEntry {
                    name: name.clone(),
                    url,
                });
            match config.save() {
                Ok(()) => println!("tracked DAT source '{name}'"),
                Err(err) => eprintln!("error: cannot save config: {err}"),
            }
        }
        DatCommands::SourceList => {
            let config = AppConfig::load().unwrap_or_default();
            if config.dat_sources.is_empty() {
                println!("No DAT sources tracked. Add one with `dat source-add <name> <url>`.");
            }
            for source in &config.dat_sources {
                println!("{} -> {}", source.name, source.url);
            }
        }
        DatCommands::SourceRemove { name } => {
            let mut config = AppConfig::load().unwrap_or_default();
            let before = config.dat_sources.len();
            config.dat_sources.retain(|s| s.name != name);
            if config.dat_sources.len() == before {
                eprintln!("warning: no DAT source named '{name}'");
                return;
            }
            match config.save() {
                Ok(()) => println!("removed DAT source '{name}'"),
                Err(err) => eprintln!("error: cannot save config: {err}"),
            }
        }
        DatCommands::Update { name } => {
            let config = AppConfig::load().unwrap_or_default();
            let Some(entry) = config.dat_sources.iter().find(|s| s.name == name) else {
                eprintln!("error: no DAT source named '{name}'");
                return;
            };
            run_single_dat_update(entry);
        }
        DatCommands::UpdateAll => {
            let config = AppConfig::load().unwrap_or_default();
            if config.dat_sources.is_empty() {
                println!("No DAT sources tracked.");
                return;
            }
            for entry in &config.dat_sources {
                run_single_dat_update(entry);
            }
        }
        DatCommands::DetectMissing {
            roms_root,
            dat_dir,
            assist,
        } => run_detect_missing_command(roms_root, dat_dir, assist),
    }
}

fn run_detect_missing_command(roms_root: PathBuf, dat_dir: PathBuf, assist: bool) {
    let mut library = retrotools_core::DatLibrary::new();
    match library.import_dir(&dat_dir) {
        Ok(results) => {
            for result in results {
                if let Err(err) = result {
                    eprintln!("warning: {err}");
                }
            }
        }
        Err(err) => {
            eprintln!(
                "error: cannot read DAT directory '{}': {err}",
                dat_dir.display()
            );
            return;
        }
    }

    let missing = match retrotools_core::platforms_missing_dat(&roms_root, &library) {
        Ok(missing) => missing,
        Err(err) => {
            eprintln!(
                "error: cannot read ROMs directory '{}': {err}",
                roms_root.display()
            );
            return;
        }
    };

    if missing.is_empty() {
        println!(
            "Every ROM folder under '{}' has a matching DAT.",
            roms_root.display()
        );
        return;
    }

    println!("{} platform folder(s) without a DAT:", missing.len());
    let config = AppConfig::load().unwrap_or_default();
    for name in &missing {
        let tracked_source = config
            .dat_sources
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name));
        match (assist, tracked_source) {
            (true, Some(entry)) => {
                let source = retrotools_core::DatSource {
                    name: entry.name.clone(),
                    url: entry.url.clone(),
                };
                match retrotools_core::download_dat(&source, &dat_dir) {
                    Ok(path) => println!("  {name}: downloaded to '{}'", path.display()),
                    Err(err) => println!("  {name}: download failed: {err}"),
                }
            }
            (true, None) => println!(
                "  {name}: no tracked DAT source with a matching name; add one with `dat source-add {name} <url>`, then re-run with --assist"
            ),
            (false, Some(_)) => println!("  {name}: a tracked source with a matching name exists — re-run with --assist to download it"),
            (false, None) => println!(
                "  {name}: no DAT imported and no tracked source; add one with `dat source-add {name} <url>` or import a DAT file manually"
            ),
        }
    }
}

fn run_single_dat_update(entry: &retrotools_common::config::DatSourceEntry) {
    let source = retrotools_core::DatSource {
        name: entry.name.clone(),
        url: entry.url.clone(),
    };

    let download_dir = match retrotools_common::config::managed_dat_dir_path() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("error: cannot resolve managed DAT directory: {err}");
            return;
        }
    };

    let cache = retrotools_common::config::dat_cache_file_path()
        .and_then(|p| retrotools_core::DatCache::open(&p))
        .ok();
    let previous_version = cache
        .as_ref()
        .and_then(|c| c.load_by_platform(&entry.name).ok().flatten())
        .map(|g| g.dat_version);

    match retrotools_core::check_for_update(&source, &download_dir, previous_version.as_deref()) {
        Ok(report) => {
            if report.changed {
                println!(
                    "{}: updated {} -> {} ({} games)",
                    report.name,
                    report.previous_version.as_deref().unwrap_or("none"),
                    report.new_version,
                    report.gameset.games.len()
                );
                if let Some(cache) = &cache {
                    if let Err(err) = cache.store(&report.file_path, &report.gameset) {
                        eprintln!(
                            "warning: fetched but could not cache '{}': {err}",
                            report.name
                        );
                    }
                }
            } else {
                println!("{}: up to date ({})", report.name, report.new_version);
            }
        }
        Err(err) => eprintln!("{}: error: {err}", entry.name),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_scan_command(
    root: PathBuf,
    dat: Option<PathBuf>,
    csv: Option<PathBuf>,
    html: Option<PathBuf>,
    pdf: Option<PathBuf>,
    no_recursive: bool,
    no_archives: bool,
    no_cache: bool,
) {
    let options = retrotools_core::ScanOptions {
        roots: vec![root],
        recursive: !no_recursive,
        scan_inside_archives: !no_archives,
    };

    let cache = if no_cache {
        None
    } else {
        match retrotools_common::config::scan_cache_file_path()
            .and_then(|p| retrotools_core::ScanCache::open(&p))
        {
            Ok(cache) => Some(cache),
            Err(err) => {
                eprintln!("warning: could not open scan cache ({err}), continuing without it");
                None
            }
        }
    };

    let outcome = match retrotools_core::scan(&options, cache.as_ref(), None) {
        Ok(outcome) => outcome,
        Err(err) => {
            eprintln!("error: {err}");
            return;
        }
    };

    println!(
        "Scanned {} file(s), {} error(s)",
        outcome.roms.len(),
        outcome.errors.len()
    );
    for err in &outcome.errors {
        eprintln!(
            "  error: {}{} -> {}",
            err.source_path.display(),
            err.archive_entry
                .as_deref()
                .map(|e| format!(" [{e}]"))
                .unwrap_or_default(),
            err.message
        );
    }

    let Some(dat_path) = dat else {
        for rom in &outcome.roms {
            println!(
                "  {}{} crc32={} size={}",
                rom.source_path.display(),
                rom.archive_entry
                    .as_deref()
                    .map(|e| format!(" [{e}]"))
                    .unwrap_or_default(),
                rom.hashes.crc32,
                rom.hashes.size
            );
        }
        return;
    };

    let gameset = match retrotools_core::dat::parse_dat_file(&dat_path) {
        Ok(gameset) => gameset,
        Err(err) => {
            eprintln!("error: cannot parse DAT '{}': {err}", dat_path.display());
            return;
        }
    };

    let match_report = retrotools_core::match_scan(&gameset, &outcome.roms);
    println!(
        "Matched={} Corrupt={} Unknown={} Missing={} Completion={:.1}%",
        match_report.matched.len(),
        match_report.corrupt.len(),
        match_report.unknown.len(),
        match_report.missing.len(),
        match_report.completion_percent()
    );

    let report = retrotools_core::ScanReport::new(match_report, outcome.errors);

    if let Some(csv_path) = csv {
        if let Err(err) = std::fs::write(&csv_path, report.to_csv()) {
            eprintln!("error: cannot write CSV report: {err}");
        } else {
            println!("CSV report written to {}", csv_path.display());
        }
    }
    if let Some(html_path) = html {
        if let Err(err) = std::fs::write(&html_path, report.to_html()) {
            eprintln!("error: cannot write HTML report: {err}");
        } else {
            println!("HTML report written to {}", html_path.display());
        }
    }
    if let Some(pdf_path) = pdf {
        match report.to_pdf() {
            Ok(bytes) => match std::fs::write(&pdf_path, bytes) {
                Ok(()) => println!("PDF report written to {}", pdf_path.display()),
                Err(err) => eprintln!("error: cannot write PDF report: {err}"),
            },
            Err(err) => eprintln!("error: cannot render PDF report: {err}"),
        }
    }
}

fn resolve_profile(name: Option<&str>) -> retrotools_core::RulePriority {
    let Some(name) = name else {
        return retrotools_core::RulePriority::default();
    };

    if let Some(preset) = retrotools_core::built_in_presets()
        .into_iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
    {
        return preset.rules;
    }

    match retrotools_common::config::profiles_dir_path()
        .map(retrotools_core::ProfileStore::new)
        .and_then(|store| store.load(name))
    {
        Ok(profile) => profile.rules,
        Err(err) => {
            eprintln!("warning: profile '{name}' not found ({err}), using default rules instead");
            retrotools_core::RulePriority::default()
        }
    }
}

fn run_select1g1r_command(dat: PathBuf, profile: Option<String>, verbose: bool) {
    let gameset = match retrotools_core::dat::parse_dat_file(&dat) {
        Ok(gameset) => gameset,
        Err(err) => {
            eprintln!("error: cannot parse DAT '{}': {err}", dat.display());
            return;
        }
    };

    let rules = resolve_profile(profile.as_deref());
    let preview = retrotools_core::preview_selection(&gameset.games, &rules);

    println!(
        "{}: {} kept, {} removed (out of {} total entries)",
        gameset.platform,
        preview.kept.len(),
        preview.removed.len(),
        gameset.games.len()
    );

    for explanation in &preview.explanations {
        println!(
            "  [{}] kept '{}' — {}",
            explanation.family, explanation.chosen, explanation.reason
        );
    }

    if verbose {
        println!("Kept:");
        for game in &preview.kept {
            println!("  + {}", game.name);
        }
        println!("Removed:");
        for game in &preview.removed {
            println!("  - {}", game.name);
        }
    }
}

fn run_profile_command(command: ProfileCommands) {
    match command {
        ProfileCommands::List => {
            println!("Built-in presets:");
            for preset in retrotools_core::built_in_presets() {
                println!("  {}", preset.name);
            }

            match retrotools_common::config::profiles_dir_path()
                .map(retrotools_core::ProfileStore::new)
                .and_then(|store| store.list())
            {
                Ok(profiles) if profiles.is_empty() => {
                    println!("No saved profiles.");
                }
                Ok(profiles) => {
                    println!("Saved profiles:");
                    for profile in profiles {
                        println!(
                            "  {} ({})",
                            profile.name,
                            profile.platform.as_deref().unwrap_or("any platform")
                        );
                    }
                }
                Err(err) => eprintln!("warning: cannot list saved profiles: {err}"),
            }
        }
        ProfileCommands::Show { name } => {
            let rules = resolve_profile(Some(&name));
            println!("{name}:");
            println!("  region_order: {:?}", rules.region_order);
            println!("  language_order: {:?}", rules.language_order);
            println!("  prefer_parent: {}", rules.prefer_parent);
            println!(
                "  excludes: beta={} proto={} demo={} kiosk={} promo={} unlicensed={} pirate={} bad_dump={}",
                rules.exclude_beta,
                rules.exclude_proto,
                rules.exclude_demo,
                rules.exclude_kiosk,
                rules.exclude_promo,
                rules.exclude_unlicensed,
                rules.exclude_pirate,
                rules.exclude_bad_dump
            );
        }
    }
}

fn scan_and_match(
    root: &std::path::Path,
    dat: &std::path::Path,
    no_cache: bool,
) -> Option<(
    retrotools_core::GameSet,
    retrotools_core::ScanOutcome,
    retrotools_core::MatchReport,
)> {
    let gameset = match retrotools_core::dat::parse_dat_file(dat) {
        Ok(gameset) => gameset,
        Err(err) => {
            eprintln!("error: cannot parse DAT '{}': {err}", dat.display());
            return None;
        }
    };

    let options = retrotools_core::ScanOptions {
        roots: vec![root.to_path_buf()],
        recursive: true,
        scan_inside_archives: true,
    };

    let cache = if no_cache {
        None
    } else {
        match retrotools_common::config::scan_cache_file_path()
            .and_then(|p| retrotools_core::ScanCache::open(&p))
        {
            Ok(cache) => Some(cache),
            Err(err) => {
                eprintln!("warning: could not open scan cache ({err}), continuing without it");
                None
            }
        }
    };

    let outcome = match retrotools_core::scan(&options, cache.as_ref(), None) {
        Ok(outcome) => outcome,
        Err(err) => {
            eprintln!("error: {err}");
            return None;
        }
    };
    for err in &outcome.errors {
        eprintln!(
            "  warning: {}{} -> {}",
            err.source_path.display(),
            err.archive_entry
                .as_deref()
                .map(|e| format!(" [{e}]"))
                .unwrap_or_default(),
            err.message
        );
    }

    let match_report = retrotools_core::match_scan(&gameset, &outcome.roms);
    Some((gameset, outcome, match_report))
}

fn open_undo_log() -> Option<retrotools_core::UndoLog> {
    match retrotools_common::config::undo_log_file_path()
        .and_then(|p| retrotools_core::UndoLog::open(&p))
    {
        Ok(log) => Some(log),
        Err(err) => {
            eprintln!("warning: could not open undo log ({err}), operations will not be recorded");
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_build1g1r_command(
    root: PathBuf,
    dat: PathBuf,
    dest: PathBuf,
    profile: Option<String>,
    mode: retrotools_core::TransferMode,
    organize: retrotools_core::OrganizeBy,
    rename_to_dat_name: bool,
    dry_run: bool,
    verify: bool,
    no_cache: bool,
) {
    let Some((gameset, _outcome, match_report)) = scan_and_match(&root, &dat, no_cache) else {
        return;
    };

    let rules = resolve_profile(profile.as_deref());
    let preview = retrotools_core::preview_selection(&gameset.games, &rules);
    let kept_names: std::collections::HashSet<&str> =
        preview.kept.iter().map(|g| g.name.as_str()).collect();
    let mut selected_report = match_report.clone();
    selected_report.matched.retain(|m| {
        m.matched_game
            .as_deref()
            .is_some_and(|g| kept_names.contains(g))
    });

    let options = retrotools_core::BuildOptions {
        destination_root: dest,
        mode,
        organize,
        rename_to_dat_name,
    };
    let plans = retrotools_core::plan_build(&gameset, &selected_report, &options);
    println!("{} file(s) planned for transfer", plans.len());

    let undo_log = if dry_run { None } else { open_undo_log() };
    let label = format!("build1g1r {}", gameset.platform);
    let (outcomes, batch_id) =
        match retrotools_core::execute_build(&plans, dry_run, verify, undo_log.as_ref(), &label) {
            Ok(result) => result,
            Err(err) => {
                eprintln!("error: {err}");
                return;
            }
        };

    let mut ok = 0;
    let mut failed = 0;
    for outcome in &outcomes {
        if let Some(err) = &outcome.error {
            failed += 1;
            eprintln!("  error: {} -> {}", outcome.plan.source.display(), err);
        } else if outcome.performed {
            ok += 1;
            if outcome.verified == Some(false) {
                eprintln!(
                    "  warning: {} verification mismatch",
                    outcome.plan.destination.display()
                );
            }
        }
    }

    if dry_run {
        for plan in &plans {
            println!(
                "  {} -> {}",
                plan.source.display(),
                plan.destination.display()
            );
        }
    } else {
        println!("{ok} transferred, {failed} failed");
        if let Some(batch_id) = batch_id {
            println!("Undo batch id: {batch_id}");
        }
    }
}

fn run_rebuild1g1r_command(
    root: PathBuf,
    dat: PathBuf,
    dest: PathBuf,
    profile: Option<String>,
    dry_run: bool,
    no_cache: bool,
) {
    let Some((gameset, _outcome, match_report)) = scan_and_match(&root, &dat, no_cache) else {
        return;
    };

    let rules = resolve_profile(profile.as_deref());
    let preview = retrotools_core::preview_selection(&gameset.games, &rules);
    let kept_names: std::collections::HashSet<&str> =
        preview.kept.iter().map(|g| g.name.as_str()).collect();
    let mut selected_report = match_report;
    selected_report.matched.retain(|m| {
        m.matched_game
            .as_deref()
            .is_some_and(|g| kept_names.contains(g))
    });

    match retrotools_core::rebuild_to_archives(
        &selected_report,
        &dest,
        retrotools_core::RebuildFormat::Zip,
        dry_run,
    ) {
        Ok(outcomes) => {
            for outcome in &outcomes {
                match &outcome.error {
                    Some(err) => eprintln!("  error: {} -> {err}", outcome.game_name),
                    None => println!(
                        "  {} -> {} ({} rom(s))",
                        outcome.game_name,
                        outcome.archive_path.display(),
                        outcome.rom_count
                    ),
                }
            }
            println!("{} archive(s) planned/written", outcomes.len());
        }
        Err(err) => eprintln!("error: {err}"),
    }
}

fn run_clean_command(
    root: PathBuf,
    dat: PathBuf,
    duplicates: bool,
    no_unknown: bool,
    dry_run: bool,
    no_cache: bool,
) {
    let Some((_gameset, _outcome, match_report)) = scan_and_match(&root, &dat, no_cache) else {
        return;
    };

    let mut to_trash: Vec<(&std::path::Path, &'static str)> = Vec::new();
    if !no_unknown {
        for rom_match in &match_report.unknown {
            to_trash.push((&rom_match.scanned.source_path, "unneeded"));
        }
    }
    let duplicate_groups = if duplicates {
        retrotools_core::find_duplicate_matches(&match_report)
    } else {
        Vec::new()
    };
    for group in &duplicate_groups {
        for extra in &group.extra {
            to_trash.push((&extra.scanned.source_path, "duplicate"));
        }
    }

    if to_trash.is_empty() {
        println!("Nothing to clean.");
        return;
    }

    let trash_dir = match retrotools_common::config::trash_dir_path() {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("error: cannot resolve trash directory: {err}");
            return;
        }
    };

    let undo_log = if dry_run { None } else { open_undo_log() };
    let batch_id = undo_log
        .as_ref()
        .and_then(|log| log.new_batch("clean").ok());

    let mut moved = 0;
    for (source, reason) in &to_trash {
        if dry_run {
            println!("  would trash ({reason}): {}", source.display());
            continue;
        }
        let undo_ref = match (&undo_log, &batch_id) {
            (Some(log), Some(id)) => Some((log, id.as_str())),
            _ => None,
        };
        match retrotools_core::safe_delete(source, &trash_dir, undo_ref) {
            Ok(trashed) => {
                moved += 1;
                println!("  ({reason}) {} -> {}", source.display(), trashed.display());
            }
            Err(err) => eprintln!("  error: {} -> {err}", source.display()),
        }
    }

    if !dry_run {
        println!("{moved} file(s) moved to trash ({})", trash_dir.display());
        if let Some(batch_id) = batch_id {
            println!("Undo batch id: {batch_id}");
        }
    }
}

fn run_undo_command(command: UndoCommands) {
    let Some(log) = open_undo_log() else {
        return;
    };

    match command {
        UndoCommands::List => match log.list_batches() {
            Ok(batches) if batches.is_empty() => println!("No recorded batches."),
            Ok(batches) => {
                for batch in batches {
                    println!(
                        "  {} [{}] {} action(s){} — {}",
                        batch.id,
                        batch.created_at,
                        batch.action_count,
                        if batch.undone { " (undone)" } else { "" },
                        batch.label
                    );
                }
            }
            Err(err) => eprintln!("error: {err}"),
        },
        UndoCommands::Apply { batch_id } => match log.undo_batch(&batch_id) {
            Ok(outcome) => {
                println!("{} action(s) reverted", outcome.reverted);
                for err in &outcome.errors {
                    eprintln!("  error: {err}");
                }
            }
            Err(err) => eprintln!("error: {err}"),
        },
    }
}

fn run_fix_command(root: PathBuf, dat: PathBuf, csv: Option<PathBuf>, no_cache: bool) {
    let Some((_gameset, _outcome, match_report)) = scan_and_match(&root, &dat, no_cache) else {
        return;
    };

    let fix_report = retrotools_core::build_fix_report(&match_report);
    print!("{}", fix_report.to_text());

    if let Some(csv_path) = csv {
        if let Err(err) = std::fs::write(&csv_path, fix_report.to_csv()) {
            eprintln!("error: cannot write fix report CSV: {err}");
        } else {
            println!("Fix report written to {}", csv_path.display());
        }
    }
}

fn scan_only(
    root: &std::path::Path,
    no_recursive: bool,
    no_archives: bool,
) -> Option<retrotools_core::ScanOutcome> {
    let options = retrotools_core::ScanOptions {
        roots: vec![root.to_path_buf()],
        recursive: !no_recursive,
        scan_inside_archives: !no_archives,
    };
    match retrotools_core::scan(&options, None, None) {
        Ok(outcome) => Some(outcome),
        Err(err) => {
            eprintln!("error: cannot scan '{}': {err}", root.display());
            None
        }
    }
}

fn run_compare_command(before: PathBuf, after: PathBuf, no_recursive: bool, no_archives: bool) {
    let Some(before_outcome) = scan_only(&before, no_recursive, no_archives) else {
        return;
    };
    let Some(after_outcome) = scan_only(&after, no_recursive, no_archives) else {
        return;
    };

    let comparison = retrotools_core::compare_scans(&before_outcome.roms, &after_outcome.roms);
    print!("{}", comparison.to_text());
}

fn run_status_command(roms_root: PathBuf, dat_dir: PathBuf, no_cache: bool) {
    let mut library = retrotools_core::DatLibrary::new();
    let import_results = match library.import_dir(&dat_dir) {
        Ok(results) => results,
        Err(err) => {
            eprintln!(
                "error: cannot read DAT directory '{}': {err}",
                dat_dir.display()
            );
            return;
        }
    };
    for result in import_results {
        if let Err(err) = result {
            eprintln!("warning: {err}");
        }
    }

    let entries = match std::fs::read_dir(&roms_root) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!(
                "error: cannot read ROMs directory '{}': {err}",
                roms_root.display()
            );
            return;
        }
    };

    let cache = if no_cache {
        None
    } else {
        retrotools_common::config::scan_cache_file_path()
            .and_then(|p| retrotools_core::ScanCache::open(&p))
            .ok()
    };

    let mut rows: Vec<(String, f64, usize, usize, usize)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(platform_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(dat_entry) = library.find_by_platform(platform_name) else {
            continue;
        };

        let options = retrotools_core::ScanOptions {
            roots: vec![path.clone()],
            recursive: true,
            scan_inside_archives: true,
        };
        let outcome = match retrotools_core::scan(&options, cache.as_ref(), None) {
            Ok(outcome) => outcome,
            Err(err) => {
                eprintln!("warning: cannot scan '{}': {err}", path.display());
                continue;
            }
        };
        let match_report = retrotools_core::match_scan(&dat_entry.gameset, &outcome.roms);
        rows.push((
            platform_name.to_string(),
            match_report.completion_percent(),
            match_report.matched.len(),
            match_report.missing.len(),
            match_report.corrupt.len(),
        ));
    }

    rows.sort_by(|a, b| a.0.cmp(&b.0));
    if rows.is_empty() {
        println!(
            "No platform subdirectory matched a DAT in '{}'.",
            dat_dir.display()
        );
        return;
    }

    println!(
        "{:<30} {:>10} {:>8} {:>8} {:>8}",
        "Platform", "Complete", "Matched", "Missing", "Corrupt"
    );
    for (platform, completion, matched, missing, corrupt) in &rows {
        println!("{platform:<30} {completion:>9.1}% {matched:>8} {missing:>8} {corrupt:>8}");
    }
}

fn build_plugin_registry() -> retrotools_plugin_api::PluginRegistry {
    let mut registry = retrotools_plugin_api::PluginRegistry::new();
    registry.register(Box::new(retrotools_plugin_playlists::PlaylistPlugin));
    registry.register(Box::new(retrotools_plugin_playlists::CollectionsPlugin));
    registry.register(Box::new(retrotools_plugin_bios::BiosPlugin));
    registry.register(Box::new(
        retrotools_plugin_batocera_export::BatoceraExportPlugin {
            distro: retrotools_plugin_batocera_export::Distro::Batocera,
        },
    ));
    registry.register(Box::new(
        retrotools_plugin_batocera_export::BatoceraExportPlugin {
            distro: retrotools_plugin_batocera_export::Distro::Recalbox,
        },
    ));
    registry.register(Box::new(
        retrotools_plugin_batocera_export::BatoceraExportPlugin {
            distro: retrotools_plugin_batocera_export::Distro::Lakka,
        },
    ));
    registry.register(Box::new(retrotools_plugin_saves::SavesBackupPlugin));
    registry.register(Box::new(retrotools_plugin_saves::SavesRestorePlugin));
    registry.register(Box::new(
        retrotools_plugin_controllers::ControllerExportPlugin,
    ));
    registry.register(Box::new(retrotools_plugin_scraper::ScraperPlugin));
    registry.register(Box::new(retrotools_plugin_shaders::ShaderOverridesPlugin));
    registry.register(Box::new(retrotools_plugin_shaders::ShaderCleanupPlugin));
    registry.register(Box::new(retrotools_plugin_core_advisor::CoreAdvisorPlugin));
    registry.register(Box::new(
        retrotools_plugin_sdcard_imager::SdCardInjectPlugin,
    ));
    registry.register(Box::new(
        retrotools_plugin_retroachievements::RetroAchievementsPlugin,
    ));
    registry
}

fn run_check_update_command(repository_override: Option<String>) {
    let config = AppConfig::load().unwrap_or_default();
    let Some(repository) = repository_override.or(config.update_repository) else {
        eprintln!(
            "error: no update repository configured; pass --repository owner/repo or set it in Settings"
        );
        return;
    };

    use retrotools_common::updater::{
        compare_versions, GitHubReleaseSource, UpdateSource, UpdateStatus,
    };
    let source = GitHubReleaseSource::new(repository);
    match source.check_latest() {
        Ok(Some(release)) => {
            let current = current_version().version;
            match compare_versions(current, &release.version) {
                UpdateStatus::UpToDate => println!("up to date ({current})"),
                UpdateStatus::UpdateAvailable(version) => {
                    println!("update available: {current} -> {version}");
                    println!("  {}", release.download_url);
                }
                UpdateStatus::CheckFailed => {
                    unreachable!("compare_versions never returns CheckFailed")
                }
            }
        }
        Ok(None) => println!("no releases published yet"),
        Err(err) => eprintln!("error: {err}"),
    }
}

fn run_convert_command(command: ConvertCommands) {
    match command {
        ConvertCommands::ToChd { source, dest } => {
            match retrotools_core::convert_to_chd(&source, &dest) {
                Ok(path) => println!("wrote '{}'", path.display()),
                Err(err) => eprintln!("error: {err}"),
            }
        }
        ConvertCommands::FromChd { source, dest_dir } => {
            match retrotools_core::convert_from_chd(&source, &dest_dir) {
                Ok(path) => println!("wrote '{}'", path.display()),
                Err(err) => eprintln!("error: {err}"),
            }
        }
        ConvertCommands::ToRvz { source, dest } => {
            match retrotools_core::convert_to_rvz(&source, &dest) {
                Ok(path) => println!("wrote '{}'", path.display()),
                Err(err) => eprintln!("error: {err}"),
            }
        }
        ConvertCommands::FromRvz { source, dest_dir } => {
            match retrotools_core::convert_from_rvz(&source, &dest_dir) {
                Ok(path) => println!("wrote '{}'", path.display()),
                Err(err) => eprintln!("error: {err}"),
            }
        }
        ConvertCommands::ToCso { source, dest } => {
            match retrotools_core::convert_to_cso(&source, &dest) {
                Ok(path) => println!("wrote '{}'", path.display()),
                Err(err) => eprintln!("error: {err}"),
            }
        }
        ConvertCommands::FromCso { source, dest_dir } => {
            match retrotools_core::convert_from_cso(&source, &dest_dir) {
                Ok(path) => println!("wrote '{}'", path.display()),
                Err(err) => eprintln!("error: {err}"),
            }
        }
    }
}

fn run_sdcard_command(command: SdcardCommands) {
    match command {
        SdcardCommands::ListDevices => {
            let devices = retrotools_plugin_sdcard_imager::list_removable_devices();
            if devices.is_empty() {
                println!("no removable USB disks detected");
                return;
            }
            for device in devices {
                println!(
                    "{}\t{}\t{} bytes",
                    device.id, device.model, device.size_bytes
                );
            }
        }
        SdcardCommands::Verify { image, sha256, md5 } => {
            let result = match (sha256, md5) {
                (Some(expected), None) => retrotools_plugin_sdcard_imager::verify_checksum(
                    &image,
                    retrotools_plugin_sdcard_imager::ChecksumAlgorithm::Sha256,
                    &expected,
                ),
                (None, Some(expected)) => retrotools_plugin_sdcard_imager::verify_checksum(
                    &image,
                    retrotools_plugin_sdcard_imager::ChecksumAlgorithm::Md5,
                    &expected,
                ),
                _ => Err("pass exactly one of --sha256 or --md5".to_string()),
            };
            match result {
                Ok(()) => println!("checksum OK"),
                Err(err) => eprintln!("error: {err}"),
            }
        }
        SdcardCommands::Write {
            image,
            device,
            confirm,
            dry_run,
        } => {
            match retrotools_plugin_sdcard_imager::write_image(
                &image,
                Path::new(&device),
                &device,
                &confirm,
                dry_run,
            ) {
                Ok(log) => {
                    for line in log {
                        println!("{line}");
                    }
                }
                Err(err) => eprintln!("error: {err}"),
            }
        }
    }
}

fn run_plugin_command(command: PluginCommands) {
    let registry = build_plugin_registry();

    match command {
        PluginCommands::List => {
            for plugin in registry.plugins() {
                println!("{} — {}", plugin.id(), plugin.name());
                println!("  {}", plugin.description());
            }
        }
        PluginCommands::Run {
            id,
            dat,
            output,
            source,
            profile,
            dry_run,
        } => {
            let gameset = match retrotools_core::dat::parse_dat_file(&dat) {
                Ok(gameset) => gameset,
                Err(err) => {
                    eprintln!("error: cannot parse DAT '{}': {err}", dat.display());
                    return;
                }
            };

            let kept_names: Vec<String> = match profile {
                Some(profile_name) => {
                    let rules = resolve_profile(Some(&profile_name));
                    retrotools_core::preview_selection(&gameset.games, &rules)
                        .kept
                        .into_iter()
                        .map(|g| g.name)
                        .collect()
                }
                None => Vec::new(),
            };

            let ctx = retrotools_plugin_api::PluginContext {
                gameset: &gameset,
                kept_game_names: &kept_names,
                source_dir: source.as_deref(),
                output_dir: &output,
                dry_run,
                match_report: None,
            };

            match registry.run(&id, &ctx) {
                Ok(outcome) => {
                    println!("{}", outcome.summary);
                    for file in &outcome.files_written {
                        println!("  wrote {}", file.display());
                    }
                }
                Err(err) => eprintln!("error: {err}"),
            }
        }
    }
}

fn print_summary(path: &std::path::Path, gameset: &retrotools_core::GameSet) {
    println!(
        "{} -> platform='{}' type={} version='{}' games={}",
        path.display(),
        gameset.platform,
        gameset.dat_type,
        gameset.dat_version,
        gameset.games.len()
    );
}

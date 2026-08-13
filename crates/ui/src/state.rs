use retrotools_common::config::AppConfig;
use retrotools_core::{
    DatLibrary, FolderWatcher, MatchReport, RuleProfile, RulePriority, ScanOptions, ScanOutcome,
    ScanProgress, SelectionResult, TransferOutcome,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

fn default_plugin_registry() -> retrotools_plugin_api::PluginRegistry {
    let mut registry = retrotools_plugin_api::PluginRegistry::new();
    registry.register(Box::new(retrotools_plugin_playlists::PlaylistPlugin));
    registry.register(Box::new(retrotools_plugin_bios::BiosPlugin));
    registry
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

pub struct PendingToast {
    pub kind: ToastKind,
    pub message: String,
}

enum ScanMessage {
    Progress(ScanProgress),
    Done(Result<ScanOutcome, String>),
}

enum BuildMessage {
    Done(Result<(Vec<TransferOutcome>, Option<String>), String>),
}

enum DatUpdateMessage {
    Done { name: String, result: Result<String, String> },
    MissingDatDownloaded { platform_name: String, result: Result<PathBuf, String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusFilter {
    #[default]
    All,
    Matched,
    Corrupt,
    Unknown,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortColumn {
    #[default]
    Name,
    Region,
    Status,
    Size,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GamesViewMode {
    #[default]
    List,
    Grid,
}

#[derive(Default)]
pub struct GamesFilter {
    pub search: String,
    pub region: Option<String>,
    pub language: Option<String>,
    pub status: StatusFilter,
    pub sort: SortColumn,
    pub sort_ascending: bool,
    pub view_mode: GamesViewMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameScanStatus {
    Matched,
    Corrupt,
    Missing,
    NotScanned,
}

pub struct AppState {
    pub config: AppConfig,
    pub library: DatLibrary,
    pub selected_platform: Option<String>,
    pub selected_game: Option<String>,
    pub rom_root: Option<PathBuf>,
    pub scan_outcome: Option<ScanOutcome>,
    pub match_report: Option<MatchReport>,
    pub rules: RulePriority,
    pub active_profile_name: Option<String>,
    pub selection_preview: Option<SelectionResult>,
    pub games_filter: GamesFilter,
    pub scan_in_progress: bool,
    pub scan_progress: Option<ScanProgress>,
    scan_rx: Option<Receiver<ScanMessage>>,
    pub build_in_progress: bool,
    pub last_build_summary: Option<String>,
    build_rx: Option<Receiver<BuildMessage>>,
    pub pending_toasts: Vec<PendingToast>,
    pub build_destination: Option<PathBuf>,
    pub build_mode: retrotools_core::TransferMode,
    pub build_organize: retrotools_core::OrganizeBy,
    pub build_rename_to_dat_name: bool,
    pub build_dry_run: bool,
    pub watch_folder_enabled: bool,
    folder_watcher: Option<FolderWatcher>,
    last_watch_trigger: Option<Instant>,
    pub auto_rescan_interval_secs: Option<u64>,
    last_scan_started_at: Option<Instant>,
    pub plugin_registry: retrotools_plugin_api::PluginRegistry,
    pub plugin_source_dir: Option<PathBuf>,
    pub plugin_output_dir: Option<PathBuf>,
    pub plugin_last_outcomes: HashMap<String, Result<String, String>>,
    pub new_dat_source_name: String,
    pub new_dat_source_url: String,
    pub dat_sources_updating: std::collections::HashSet<String>,
    pub dat_source_last_results: HashMap<String, Result<String, String>>,
    dat_update_tx: Sender<DatUpdateMessage>,
    dat_update_rx: Receiver<DatUpdateMessage>,
    pub expert_mode: bool,
    pub wizard_step: WizardStep,
    pub command_palette_open: bool,
    pub command_palette_query: String,
    pub command_palette_selected: usize,
    pub missing_dat_platforms: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WizardStep {
    #[default]
    ChooseDat,
    Scan,
    Rules,
    Preview,
    Build,
}

impl WizardStep {
    pub fn label(&self) -> &'static str {
        match self {
            WizardStep::ChooseDat => "1. Platform",
            WizardStep::Scan => "2. Scan",
            WizardStep::Rules => "3. Rules",
            WizardStep::Preview => "4. Preview",
            WizardStep::Build => "5. Build",
        }
    }

    pub fn next(self) -> Self {
        match self {
            WizardStep::ChooseDat => WizardStep::Scan,
            WizardStep::Scan => WizardStep::Rules,
            WizardStep::Rules => WizardStep::Preview,
            WizardStep::Preview => WizardStep::Build,
            WizardStep::Build => WizardStep::Build,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            WizardStep::ChooseDat => WizardStep::ChooseDat,
            WizardStep::Scan => WizardStep::ChooseDat,
            WizardStep::Rules => WizardStep::Scan,
            WizardStep::Preview => WizardStep::Rules,
            WizardStep::Build => WizardStep::Preview,
        }
    }
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let (dat_update_tx, dat_update_rx) = std::sync::mpsc::channel();
        Self {
            config,
            library: DatLibrary::new(),
            selected_platform: None,
            selected_game: None,
            rom_root: None,
            scan_outcome: None,
            match_report: None,
            rules: RulePriority::default(),
            active_profile_name: None,
            selection_preview: None,
            games_filter: GamesFilter::default(),
            scan_in_progress: false,
            scan_progress: None,
            scan_rx: None,
            build_in_progress: false,
            last_build_summary: None,
            build_rx: None,
            pending_toasts: Vec::new(),
            build_destination: None,
            build_mode: retrotools_core::TransferMode::Copy,
            build_organize: retrotools_core::OrganizeBy::ByPlatform,
            build_rename_to_dat_name: false,
            build_dry_run: false,
            watch_folder_enabled: false,
            folder_watcher: None,
            last_watch_trigger: None,
            auto_rescan_interval_secs: None,
            last_scan_started_at: None,
            plugin_registry: default_plugin_registry(),
            plugin_source_dir: None,
            plugin_output_dir: None,
            plugin_last_outcomes: HashMap::new(),
            new_dat_source_name: String::new(),
            new_dat_source_url: String::new(),
            dat_sources_updating: std::collections::HashSet::new(),
            dat_source_last_results: HashMap::new(),
            dat_update_tx: dat_update_tx.clone(),
            dat_update_rx,
            expert_mode: true,
            wizard_step: WizardStep::default(),
            command_palette_open: false,
            command_palette_query: String::new(),
            command_palette_selected: 0,
            missing_dat_platforms: Vec::new(),
        }
    }

    pub fn add_dat_source(&mut self) {
        let name = self.new_dat_source_name.trim().to_string();
        let url = self.new_dat_source_url.trim().to_string();
        if name.is_empty() || url.is_empty() {
            self.toast(ToastKind::Warning, "Enter both a name and a URL");
            return;
        }
        self.config.dat_sources.retain(|s| s.name != name);
        self.config
            .dat_sources
            .push(retrotools_common::config::DatSourceEntry { name: name.clone(), url });
        if let Err(err) = self.config.save() {
            self.toast(ToastKind::Error, format!("Cannot save settings: {err}"));
            return;
        }
        self.new_dat_source_name.clear();
        self.new_dat_source_url.clear();
        self.toast(ToastKind::Success, format!("Tracking DAT source '{name}'"));
    }

    pub fn remove_dat_source(&mut self, name: &str) {
        self.config.dat_sources.retain(|s| s.name != name);
        if let Err(err) = self.config.save() {
            self.toast(ToastKind::Error, format!("Cannot save settings: {err}"));
        }
    }

    pub fn start_dat_source_update(&mut self, name: &str) {
        if self.dat_sources_updating.contains(name) {
            return;
        }
        let Some(entry) = self.config.dat_sources.iter().find(|s| s.name == name) else {
            return;
        };
        let source = retrotools_core::DatSource {
            name: entry.name.clone(),
            url: entry.url.clone(),
        };

        self.dat_sources_updating.insert(name.to_string());
        let tx = self.dat_update_tx.clone();

        std::thread::spawn(move || {
            let result = (|| -> Result<String, String> {
                let download_dir = retrotools_common::config::managed_dat_dir_path()
                    .map_err(|e| e.to_string())?;
                let cache = retrotools_common::config::dat_cache_file_path()
                    .and_then(|p| retrotools_core::DatCache::open(&p))
                    .ok();
                let previous_version = cache
                    .as_ref()
                    .and_then(|c| c.load_by_platform(&source.name).ok().flatten())
                    .map(|g| g.dat_version);

                let report = retrotools_core::check_for_update(
                    &source,
                    &download_dir,
                    previous_version.as_deref(),
                )
                .map_err(|e| e.to_string())?;

                if report.changed {
                    if let Some(cache) = &cache {
                        let _ = cache.store(&report.file_path, &report.gameset);
                    }
                    Ok(format!(
                        "updated {} -> {} ({} games)",
                        report.previous_version.as_deref().unwrap_or("none"),
                        report.new_version,
                        report.gameset.games.len()
                    ))
                } else {
                    Ok(format!("up to date ({})", report.new_version))
                }
            })();

            let _ = tx.send(DatUpdateMessage::Done {
                name: source.name,
                result,
            });
        });
    }

    /// Scans every configured ROM directory for subfolders that have no
    /// matching platform in `self.library` yet. There's no No-Intro/Redump
    /// discovery API to identify the right DAT automatically (same
    /// limitation as [`Self::start_dat_source_update`]), so this only flags
    /// which folders need attention; pairing one with a tracked source is
    /// what [`Self::start_missing_dat_download`] does.
    pub fn detect_missing_dats(&mut self) {
        let mut missing = std::collections::BTreeSet::new();
        for root in &self.config.rom_directories {
            if let Ok(found) = retrotools_core::platforms_missing_dat(root, &self.library) {
                missing.extend(found);
            }
        }
        self.missing_dat_platforms = missing.into_iter().collect();
        if self.missing_dat_platforms.is_empty() {
            self.toast(ToastKind::Success, "Every configured ROM directory has a matching DAT");
        }
    }

    /// Downloads the tracked DAT source whose name matches `platform_name`
    /// (case-insensitive) and imports it into the library on success.
    pub fn start_missing_dat_download(&mut self, platform_name: &str) {
        if self.dat_sources_updating.contains(platform_name) {
            return;
        }
        let Some(entry) = self
            .config
            .dat_sources
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(platform_name))
        else {
            return;
        };
        let source = retrotools_core::DatSource {
            name: entry.name.clone(),
            url: entry.url.clone(),
        };
        self.dat_sources_updating.insert(platform_name.to_string());
        let tx = self.dat_update_tx.clone();
        let platform_name = platform_name.to_string();

        std::thread::spawn(move || {
            let result = (|| -> Result<PathBuf, String> {
                let download_dir =
                    retrotools_common::config::managed_dat_dir_path().map_err(|e| e.to_string())?;
                retrotools_core::download_dat(&source, &download_dir).map_err(|e| e.to_string())
            })();
            let _ = tx.send(DatUpdateMessage::MissingDatDownloaded { platform_name, result });
        });
    }

    pub fn language(&self) -> crate::i18n::Language {
        crate::i18n::Language::from_code(&self.config.language)
    }

    pub fn t(&self, key: crate::i18n::Key) -> &'static str {
        crate::i18n::t(self.language(), key)
    }

    pub fn toast(&mut self, kind: ToastKind, message: impl Into<String>) {
        self.pending_toasts.push(PendingToast {
            kind,
            message: message.into(),
        });
    }

    pub fn run_plugin(&mut self, plugin_id: &str) {
        let Some(gameset) = self.current_gameset() else {
            self.toast(ToastKind::Warning, "Select a platform (import a DAT) before running a plugin");
            return;
        };
        let Some(output_dir) = self.plugin_output_dir.clone() else {
            self.toast(ToastKind::Warning, "Choose an output folder for the plugin first");
            return;
        };

        let kept_game_names: Vec<String> = self
            .selection_preview
            .as_ref()
            .map(|preview| preview.kept.iter().map(|g| g.name.clone()).collect())
            .unwrap_or_default();

        let ctx = retrotools_plugin_api::PluginContext {
            gameset,
            kept_game_names: &kept_game_names,
            source_dir: self.plugin_source_dir.as_deref(),
            output_dir: &output_dir,
        };

        let result = self.plugin_registry.run(plugin_id, &ctx);
        match &result {
            Ok(outcome) => self.toast(ToastKind::Success, outcome.summary.clone()),
            Err(err) => self.toast(ToastKind::Error, format!("Plugin failed: {err}")),
        }
        self.plugin_last_outcomes
            .insert(plugin_id.to_string(), result.map(|o| o.summary));
    }

    pub fn import_dat(&mut self, path: PathBuf) {
        match self.library.import_file(&path) {
            Ok(entry) => {
                let platform = entry.gameset.platform.clone();
                let games = entry.gameset.games.len();
                self.toast(
                    ToastKind::Success,
                    format!("Imported '{platform}' ({games} games)"),
                );
                self.selected_platform = Some(platform);
                self.match_report = None;
                self.selection_preview = None;
                self.scan_outcome = None;
            }
            Err(err) => self.toast(ToastKind::Error, format!("Cannot import DAT: {err}")),
        }
    }

    pub fn current_gameset(&self) -> Option<&retrotools_core::GameSet> {
        let platform = self.selected_platform.as_deref()?;
        self.library
            .find_by_platform(platform)
            .map(|entry| &entry.gameset)
    }

    /// Average throughput of the scan in progress, in (files/sec, bytes/sec),
    /// based on elapsed time since `start_scan` and the latest progress
    /// report. `None` before the first progress update arrives.
    pub fn scan_speed(&self) -> Option<(f64, f64)> {
        let progress = self.scan_progress.as_ref()?;
        let started_at = self.last_scan_started_at?;
        let elapsed = started_at.elapsed().as_secs_f64();
        if elapsed <= 0.0 {
            return None;
        }
        Some((
            progress.files_scanned as f64 / elapsed,
            progress.bytes_scanned as f64 / elapsed,
        ))
    }

    pub fn start_scan(&mut self, root: PathBuf) {
        if self.scan_in_progress {
            return;
        }
        let Some(gameset) = self.current_gameset() else {
            self.toast(ToastKind::Warning, "Select a platform (import a DAT) before scanning");
            return;
        };
        let _ = gameset;

        self.rom_root = Some(root.clone());
        self.scan_in_progress = true;
        self.scan_progress = None;
        self.last_scan_started_at = Some(Instant::now());

        let (tx, rx) = std::sync::mpsc::channel::<ScanMessage>();
        self.scan_rx = Some(rx);
        let tx_progress: Sender<ScanMessage> = tx.clone();
        let no_cache = false;

        std::thread::spawn(move || {
            let options = ScanOptions {
                roots: vec![root],
                recursive: true,
                scan_inside_archives: true,
            };

            let cache = if no_cache {
                None
            } else {
                retrotools_common::config::scan_cache_file_path()
                    .and_then(|p| retrotools_core::ScanCache::open(&p))
                    .ok()
            };

            let progress_sender = Mutex::new(tx_progress);
            let callback = move |progress: ScanProgress| {
                if let Ok(sender) = progress_sender.lock() {
                    let _ = sender.send(ScanMessage::Progress(progress));
                }
            };

            let result = retrotools_core::scan(&options, cache.as_ref(), Some(&callback))
                .map_err(|e| e.to_string());
            let _ = tx.send(ScanMessage::Done(result));
        });
    }

    pub fn set_watch_enabled(&mut self, enabled: bool) {
        self.watch_folder_enabled = enabled;
        if !enabled {
            self.folder_watcher = None;
            return;
        }
        let Some(root) = self.rom_root.clone() else {
            self.watch_folder_enabled = false;
            self.toast(ToastKind::Warning, "Choose a ROM folder before enabling the watcher");
            return;
        };
        match FolderWatcher::watch(&root) {
            Ok(watcher) => {
                self.folder_watcher = Some(watcher);
                self.toast(ToastKind::Info, format!("Watching '{}' for changes", root.display()));
            }
            Err(err) => {
                self.watch_folder_enabled = false;
                self.toast(ToastKind::Error, format!("Cannot watch folder: {err}"));
            }
        }
    }

    /// Checks the folder watcher and the periodic-rescan timer, triggering a
    /// new scan when appropriate. Called once per frame from `poll_jobs`.
    fn check_background_triggers(&mut self) {
        if self.scan_in_progress || self.selected_platform.is_none() {
            return;
        }
        let Some(root) = self.rom_root.clone() else {
            return;
        };

        const WATCH_DEBOUNCE: Duration = Duration::from_secs(3);
        if self.watch_folder_enabled {
            let has_changes = self
                .folder_watcher
                .as_ref()
                .is_some_and(|w| w.has_pending_changes());
            if has_changes {
                let debounced = self
                    .last_watch_trigger
                    .is_some_and(|t| t.elapsed() < WATCH_DEBOUNCE);
                if !debounced {
                    self.last_watch_trigger = Some(Instant::now());
                    self.toast(ToastKind::Info, "Change detected, re-scanning...");
                    self.start_scan(root.clone());
                    return;
                }
            }
        }

        if let Some(interval_secs) = self.auto_rescan_interval_secs {
            let due = self
                .last_scan_started_at
                .map_or(true, |t| t.elapsed() >= Duration::from_secs(interval_secs));
            if due {
                self.start_scan(root);
            }
        }
    }

    pub fn poll_jobs(&mut self) {
        self.check_background_triggers();
        while let Ok(message) = self.dat_update_rx.try_recv() {
            match message {
                DatUpdateMessage::Done { name, result } => {
                    self.dat_sources_updating.remove(&name);
                    match &result {
                        Ok(summary) => self.toast(ToastKind::Success, format!("{name}: {summary}")),
                        Err(err) => self.toast(ToastKind::Error, format!("{name}: {err}")),
                    }
                    self.dat_source_last_results.insert(name, result);
                }
                DatUpdateMessage::MissingDatDownloaded { platform_name, result } => {
                    self.dat_sources_updating.remove(&platform_name);
                    match result {
                        Ok(path) => match self.library.import_file(&path) {
                            Ok(_) => {
                                self.missing_dat_platforms.retain(|p| !p.eq_ignore_ascii_case(&platform_name));
                                self.toast(ToastKind::Success, format!("Imported DAT for '{platform_name}'"));
                            }
                            Err(err) => self.toast(
                                ToastKind::Error,
                                format!("{platform_name}: downloaded but failed to parse: {err}"),
                            ),
                        },
                        Err(err) => self.toast(ToastKind::Error, format!("{platform_name}: {err}")),
                    }
                }
            }
        }
        if let Some(rx) = &self.scan_rx {
            let mut done_result = None;
            while let Ok(message) = rx.try_recv() {
                match message {
                    ScanMessage::Progress(progress) => self.scan_progress = Some(progress),
                    ScanMessage::Done(result) => done_result = Some(result),
                }
            }
            if let Some(result) = done_result {
                self.scan_in_progress = false;
                self.scan_rx = None;
                match result {
                    Ok(outcome) => {
                        let matched_count = outcome.roms.len();
                        let error_count = outcome.errors.len();
                        if let Some(gameset) = self.current_gameset() {
                            self.match_report = Some(retrotools_core::match_scan(gameset, &outcome.roms));
                        }
                        self.scan_outcome = Some(outcome);
                        self.toast(
                            ToastKind::Success,
                            format!("Scan complete: {matched_count} file(s), {error_count} error(s)"),
                        );
                    }
                    Err(err) => self.toast(ToastKind::Error, format!("Scan failed: {err}")),
                }
            }
        }

        if let Some(rx) = &self.build_rx {
            let mut done_result = None;
            while let Ok(message) = rx.try_recv() {
                match message {
                    BuildMessage::Done(result) => done_result = Some(result),
                }
            }
            if let Some(result) = done_result {
                self.build_in_progress = false;
                self.build_rx = None;
                match result {
                    Ok((outcomes, batch_id)) => {
                        let ok = outcomes.iter().filter(|o| o.performed && o.error.is_none()).count();
                        let failed = outcomes.iter().filter(|o| o.error.is_some()).count();
                        self.last_build_summary = Some(format!(
                            "{ok} transferred, {failed} failed{}",
                            batch_id
                                .as_ref()
                                .map(|b| format!(" (undo batch: {b})"))
                                .unwrap_or_default()
                        ));
                        self.toast(
                            ToastKind::Success,
                            format!("Build complete: {ok} transferred, {failed} failed"),
                        );
                    }
                    Err(err) => self.toast(ToastKind::Error, format!("Build failed: {err}")),
                }
            }
        }
    }

    pub fn run_preview(&mut self) {
        let Some(gameset) = self.current_gameset() else {
            return;
        };
        self.selection_preview = Some(retrotools_core::preview_selection(&gameset.games, &self.rules));
        self.toast(ToastKind::Info, "1G1R preview updated");
    }

    pub fn load_profile(&mut self, profile: RuleProfile) {
        self.rules = profile.rules;
        self.active_profile_name = Some(profile.name);
        self.selection_preview = None;
    }

    pub fn save_profile(&mut self, name: String) {
        let store = match retrotools_common::config::profiles_dir_path() {
            Ok(dir) => retrotools_core::ProfileStore::new(dir),
            Err(err) => {
                self.toast(ToastKind::Error, format!("Cannot resolve profiles directory: {err}"));
                return;
            }
        };
        let profile = RuleProfile::new(name.clone(), self.selected_platform.clone(), self.rules.clone());
        match store.save(&profile) {
            Ok(_) => {
                self.active_profile_name = Some(name.clone());
                self.toast(ToastKind::Success, format!("Profile '{name}' saved"));
            }
            Err(err) => self.toast(ToastKind::Error, format!("Cannot save profile: {err}")),
        }
    }

    pub fn saved_profiles(&mut self) -> Vec<RuleProfile> {
        match retrotools_common::config::profiles_dir_path()
            .map(retrotools_core::ProfileStore::new)
            .and_then(|store| store.list())
        {
            Ok(profiles) => profiles,
            Err(err) => {
                self.toast(ToastKind::Warning, format!("Cannot list saved profiles: {err}"));
                Vec::new()
            }
        }
    }

    pub fn start_build(
        &mut self,
        destination: PathBuf,
        mode: retrotools_core::TransferMode,
        organize: retrotools_core::OrganizeBy,
        rename_to_dat_name: bool,
        dry_run: bool,
    ) {
        if self.build_in_progress {
            return;
        }
        let (Some(gameset), Some(match_report), Some(preview)) = (
            self.current_gameset().cloned(),
            self.match_report.clone(),
            self.selection_preview.clone(),
        ) else {
            self.toast(
                ToastKind::Warning,
                "Scan a ROM directory and preview the 1G1R selection first",
            );
            return;
        };

        let kept_names: std::collections::HashSet<String> =
            preview.kept.iter().map(|g| g.name.clone()).collect();
        let mut selected_report = match_report;
        selected_report
            .matched
            .retain(|m| m.matched_game.as_deref().is_some_and(|g| kept_names.contains(g)));

        self.build_in_progress = true;
        let (tx, rx) = std::sync::mpsc::channel::<BuildMessage>();
        self.build_rx = Some(rx);
        let platform = gameset.platform.clone();

        std::thread::spawn(move || {
            let options = retrotools_core::BuildOptions {
                destination_root: destination,
                mode,
                organize,
                rename_to_dat_name,
            };
            let plans = retrotools_core::plan_build(&gameset, &selected_report, &options);

            let undo_log = if dry_run {
                None
            } else {
                retrotools_common::config::undo_log_file_path()
                    .and_then(|p| retrotools_core::UndoLog::open(&p))
                    .ok()
            };
            let label = format!("build1g1r {platform}");
            let result = retrotools_core::execute_build(&plans, dry_run, true, undo_log.as_ref(), &label)
                .map_err(|e| e.to_string());
            let _ = tx.send(BuildMessage::Done(result));
        });
    }

    pub fn game_scan_status(&self, game_name: &str) -> GameScanStatus {
        let Some(report) = &self.match_report else {
            return GameScanStatus::NotScanned;
        };
        if report
            .matched
            .iter()
            .any(|m| m.matched_game.as_deref() == Some(game_name))
        {
            return GameScanStatus::Matched;
        }
        if report
            .corrupt
            .iter()
            .any(|m| m.matched_game.as_deref() == Some(game_name))
        {
            return GameScanStatus::Corrupt;
        }
        if report.missing.iter().any(|m| m.game_name == game_name) {
            return GameScanStatus::Missing;
        }
        GameScanStatus::NotScanned
    }

    pub fn is_kept(&self, game_name: &str) -> Option<bool> {
        self.selection_preview
            .as_ref()
            .map(|preview| preview.kept.iter().any(|g| g.name == game_name))
    }

    pub fn platform_stats(&self) -> HashMap<String, usize> {
        self.library
            .entries()
            .iter()
            .map(|entry| (entry.gameset.platform.clone(), entry.gameset.games.len()))
            .collect()
    }
}

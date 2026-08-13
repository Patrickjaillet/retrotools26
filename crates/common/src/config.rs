use crate::error::{AppError, AppResult};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ThemePreference {
    Light,
    Dark,
    System,
}

impl Default for ThemePreference {
    fn default() -> Self {
        ThemePreference::System
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatSourceEntry {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub theme: ThemePreference,
    pub accent_color: [u8; 3],
    pub language: String,
    pub rom_directories: Vec<PathBuf>,
    pub dat_directories: Vec<PathBuf>,
    pub last_selected_platform: Option<String>,
    pub check_updates_on_startup: bool,
    /// `"owner/repo"` to check GitHub Releases against on startup. `None`
    /// (the default) disables the check entirely rather than pointing at a
    /// guessed repository — set once the project has a real GitHub remote.
    #[serde(default)]
    pub update_repository: Option<String>,
    pub log_level: String,
    #[serde(default)]
    pub dat_sources: Vec<DatSourceEntry>,
    /// UI scale factor (egui `pixels_per_point` multiplier). 1.0 is the
    /// platform default; higher values enlarge text and widgets for
    /// accessibility.
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
}

fn default_ui_scale() -> f32 {
    1.0
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: ThemePreference::default(),
            accent_color: [0, 122, 204],
            language: "en".to_string(),
            rom_directories: Vec::new(),
            dat_directories: Vec::new(),
            last_selected_platform: None,
            check_updates_on_startup: true,
            update_repository: None,
            log_level: "info".to_string(),
            dat_sources: Vec::new(),
            ui_scale: default_ui_scale(),
        }
    }
}

pub fn project_dirs() -> AppResult<ProjectDirs> {
    ProjectDirs::from("com", "RetroTools", "RetroTools2026")
        .ok_or_else(|| AppError::Config("unable to resolve platform config directory".into()))
}

/// Portable mode: if a `portable.txt` file sits next to the running
/// executable, every path this module resolves lives under `<exe_dir>/data`
/// instead of the per-user `%APPDATA%`/`%LOCALAPPDATA%` directories —
/// dropping the whole folder onto a USB stick (or deleting it) carries every
/// setting/cache/log with it, no installer or registry state involved. This
/// is checked fresh every call rather than cached, so tests can toggle it by
/// creating/removing the marker file.
pub fn is_portable_mode() -> bool {
    portable_marker_path().map(|p| p.is_file()).unwrap_or(false)
}

fn portable_marker_path() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("portable.txt")))
}

fn portable_data_root() -> AppResult<PathBuf> {
    let exe = std::env::current_exe().map_err(AppError::Io)?;
    let dir = exe
        .parent()
        .ok_or_else(|| AppError::Config("cannot resolve the executable's directory".into()))?;
    Ok(dir.join("data"))
}

fn config_base_dir() -> AppResult<PathBuf> {
    if is_portable_mode() {
        portable_data_root()
    } else {
        Ok(project_dirs()?.config_dir().to_path_buf())
    }
}

fn data_base_dir() -> AppResult<PathBuf> {
    if is_portable_mode() {
        portable_data_root()
    } else {
        Ok(project_dirs()?.data_local_dir().to_path_buf())
    }
}

pub fn config_file_path() -> AppResult<PathBuf> {
    Ok(config_base_dir()?.join("config.toml"))
}

pub fn log_dir_path() -> AppResult<PathBuf> {
    Ok(data_base_dir()?.join("logs"))
}

pub fn dat_cache_file_path() -> AppResult<PathBuf> {
    Ok(data_base_dir()?.join("cache").join("dat_cache.sqlite3"))
}

pub fn scan_cache_file_path() -> AppResult<PathBuf> {
    Ok(data_base_dir()?.join("cache").join("scan_cache.sqlite3"))
}

pub fn profiles_dir_path() -> AppResult<PathBuf> {
    Ok(data_base_dir()?.join("profiles"))
}

pub fn undo_log_file_path() -> AppResult<PathBuf> {
    Ok(data_base_dir()?.join("cache").join("undo_log.sqlite3"))
}

pub fn trash_dir_path() -> AppResult<PathBuf> {
    Ok(data_base_dir()?.join("trash"))
}

pub fn managed_dat_dir_path() -> AppResult<PathBuf> {
    Ok(data_base_dir()?.join("dats"))
}

/// Per-plugin editable data (correspondence tables, cached lookups, etc.),
/// one subfolder per plugin id so unrelated plugins never collide —
/// e.g. `plugin_data_dir_path("batocera-export")`.
pub fn plugin_data_dir_path(plugin_id: &str) -> AppResult<PathBuf> {
    Ok(data_base_dir()?.join("plugins").join(plugin_id))
}

impl AppConfig {
    pub fn load() -> AppResult<Self> {
        let path = config_file_path()?;
        if !path.exists() {
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }
        let raw = std::fs::read_to_string(&path)?;
        let config: AppConfig = toml::from_str(&raw)?;
        Ok(config)
    }

    pub fn save(&self) -> AppResult<()> {
        let path = config_file_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = toml::to_string_pretty(self)?;
        std::fs::write(&path, raw)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // `is_portable_mode` reads a real file next to the test binary's own
    // `current_exe()`, so these two tests can't run concurrently with each
    // other without racing on that shared marker file.
    static PORTABLE_MARKER_GUARD: Mutex<()> = Mutex::new(());

    #[test]
    fn portable_mode_is_off_by_default() {
        let _guard = PORTABLE_MARKER_GUARD.lock().unwrap();
        let marker = portable_marker_path().unwrap();
        std::fs::remove_file(&marker).ok();
        assert!(!is_portable_mode());
    }

    #[test]
    fn creating_the_marker_next_to_the_exe_switches_every_path_under_exe_dir_data() {
        let _guard = PORTABLE_MARKER_GUARD.lock().unwrap();
        let marker = portable_marker_path().unwrap();
        std::fs::write(&marker, b"").unwrap();

        assert!(is_portable_mode());
        let expected_root = marker.parent().unwrap().join("data");
        assert_eq!(config_file_path().unwrap(), expected_root.join("config.toml"));
        assert_eq!(log_dir_path().unwrap(), expected_root.join("logs"));
        assert_eq!(trash_dir_path().unwrap(), expected_root.join("trash"));

        std::fs::remove_file(&marker).unwrap();
        assert!(!is_portable_mode());
    }
}

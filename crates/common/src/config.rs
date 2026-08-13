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

pub fn config_file_path() -> AppResult<PathBuf> {
    let dirs = project_dirs()?;
    Ok(dirs.config_dir().join("config.toml"))
}

pub fn log_dir_path() -> AppResult<PathBuf> {
    let dirs = project_dirs()?;
    Ok(dirs.data_local_dir().join("logs"))
}

pub fn dat_cache_file_path() -> AppResult<PathBuf> {
    let dirs = project_dirs()?;
    Ok(dirs.data_local_dir().join("cache").join("dat_cache.sqlite3"))
}

pub fn scan_cache_file_path() -> AppResult<PathBuf> {
    let dirs = project_dirs()?;
    Ok(dirs.data_local_dir().join("cache").join("scan_cache.sqlite3"))
}

pub fn profiles_dir_path() -> AppResult<PathBuf> {
    let dirs = project_dirs()?;
    Ok(dirs.data_local_dir().join("profiles"))
}

pub fn undo_log_file_path() -> AppResult<PathBuf> {
    let dirs = project_dirs()?;
    Ok(dirs.data_local_dir().join("cache").join("undo_log.sqlite3"))
}

pub fn trash_dir_path() -> AppResult<PathBuf> {
    let dirs = project_dirs()?;
    Ok(dirs.data_local_dir().join("trash"))
}

pub fn managed_dat_dir_path() -> AppResult<PathBuf> {
    let dirs = project_dirs()?;
    Ok(dirs.data_local_dir().join("dats"))
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

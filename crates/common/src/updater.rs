use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub version: String,
    pub download_url: String,
    pub published_at: chrono::DateTime<chrono::Utc>,
    pub release_notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStatus {
    UpToDate,
    UpdateAvailable(String),
    CheckFailed,
}

pub trait UpdateSource {
    fn check_latest(&self) -> AppResult<Option<ReleaseInfo>>;
}

pub struct GitHubReleaseSource {
    pub repository: String,
}

impl UpdateSource for GitHubReleaseSource {
    fn check_latest(&self) -> AppResult<Option<ReleaseInfo>> {
        Err(AppError::Update("update source not yet implemented".into()))
    }
}

pub fn compare_versions(current: &str, remote: &str) -> UpdateStatus {
    if current == remote {
        UpdateStatus::UpToDate
    } else {
        UpdateStatus::UpdateAvailable(remote.to_string())
    }
}

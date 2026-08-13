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

/// Checks GitHub's public Releases API for `owner/repo`'s latest release —
/// no auth token needed (unauthenticated requests are just more heavily
/// rate-limited), which is what makes this practical as a startup check
/// baked into a distributed binary.
pub struct GitHubReleaseSource {
    /// `"owner/repo"`, e.g. `"patrickjaillet/retrotools26"`.
    pub repository: String,
    /// API base URL, defaulting to `https://api.github.com`. Overridable so
    /// tests can point this at a local server instead of making real
    /// network calls — see [`GitHubReleaseSource::new`].
    pub api_base_url: String,
}

impl GitHubReleaseSource {
    pub fn new(repository: impl Into<String>) -> Self {
        Self {
            repository: repository.into(),
            api_base_url: "https://api.github.com".to_string(),
        }
    }
}

impl UpdateSource for GitHubReleaseSource {
    fn check_latest(&self) -> AppResult<Option<ReleaseInfo>> {
        let url = format!("{}/repos/{}/releases/latest", self.api_base_url, self.repository);
        let response = match ureq::get(&url)
            .set("User-Agent", "retrotools2026-updater")
            .set("Accept", "application/vnd.github+json")
            .call()
        {
            Ok(response) => response,
            // A brand new repository with no releases yet is a legitimate
            // state, not a failure worth surfacing as an error toast.
            Err(ureq::Error::Status(404, _)) => return Ok(None),
            Err(err) => return Err(AppError::Update(format!("cannot reach GitHub releases API: {err}"))),
        };

        let body = response
            .into_string()
            .map_err(|err| AppError::Update(format!("cannot read GitHub releases response: {err}")))?;
        let json: serde_json::Value = serde_json::from_str(&body)
            .map_err(|err| AppError::Update(format!("malformed GitHub releases response: {err}")))?;

        let version = json
            .get("tag_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Update("GitHub release response has no tag_name".into()))?
            .trim_start_matches('v')
            .to_string();

        let published_at = json
            .get("published_at")
            .and_then(|v| v.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(chrono::Utc::now);

        let release_notes = json.get("body").and_then(|v| v.as_str()).unwrap_or_default().to_string();

        // Prefer a real installer/portable-zip asset over the generic
        // "view this release on GitHub" page, but fall back to the page if
        // no asset was attached yet.
        let download_url = json
            .get("assets")
            .and_then(|v| v.as_array())
            .and_then(|assets| {
                assets.iter().find(|asset| {
                    asset
                        .get("name")
                        .and_then(|n| n.as_str())
                        .map(|n| n.ends_with(".exe") || n.ends_with(".msi") || n.ends_with(".zip"))
                        .unwrap_or(false)
                })
            })
            .and_then(|asset| asset.get("browser_download_url"))
            .and_then(|v| v.as_str())
            .or_else(|| json.get("html_url").and_then(|v| v.as_str()))
            .unwrap_or_default()
            .to_string();

        Ok(Some(ReleaseInfo {
            version,
            download_url,
            published_at,
            release_notes,
        }))
    }
}

pub fn compare_versions(current: &str, remote: &str) -> UpdateStatus {
    if current == remote {
        UpdateStatus::UpToDate
    } else {
        UpdateStatus::UpdateAvailable(remote.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn same_version_is_up_to_date() {
        assert_eq!(compare_versions("1.2.3", "1.2.3"), UpdateStatus::UpToDate);
    }

    #[test]
    fn different_version_is_an_update() {
        assert_eq!(
            compare_versions("1.2.3", "1.3.0"),
            UpdateStatus::UpdateAvailable("1.3.0".to_string())
        );
    }

    /// Spins up a real (loopback-only) HTTP/1.1 server that serves exactly
    /// one request with `body`, then shuts down. Used instead of mocking
    /// `ureq` so `check_latest` is exercised against a real socket, same as
    /// the DAT-auto-update tests in `retrotools_core::dat_update` were
    /// validated against a real local server during development.
    fn serve_once(status_line: &str, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let status_line = status_line.to_string();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf); // drain the request, ignore its content
            let response = format!(
                "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        });
        format!("http://127.0.0.1:{port}")
    }

    #[test]
    fn parses_a_real_github_release_response_from_a_local_server() {
        let body = r#"{
            "tag_name": "v1.4.0",
            "published_at": "2026-05-01T12:00:00Z",
            "body": "Release notes go here.",
            "html_url": "https://github.com/example/repo/releases/tag/v1.4.0",
            "assets": [
                {"name": "retrotools2026-setup.exe", "browser_download_url": "https://github.com/example/repo/releases/download/v1.4.0/retrotools2026-setup.exe"}
            ]
        }"#;
        let base_url = serve_once("200 OK", body);

        let source = GitHubReleaseSource {
            repository: "example/repo".to_string(),
            api_base_url: base_url,
        };
        let release = source.check_latest().unwrap().expect("a release was returned");
        assert_eq!(release.version, "1.4.0");
        assert_eq!(release.download_url, "https://github.com/example/repo/releases/download/v1.4.0/retrotools2026-setup.exe");
        assert_eq!(release.release_notes, "Release notes go here.");
    }

    #[test]
    fn falls_back_to_the_release_page_when_no_installer_asset_is_attached() {
        let body = r#"{
            "tag_name": "v1.4.0",
            "published_at": "2026-05-01T12:00:00Z",
            "body": "",
            "html_url": "https://github.com/example/repo/releases/tag/v1.4.0",
            "assets": []
        }"#;
        let base_url = serve_once("200 OK", body);

        let source = GitHubReleaseSource {
            repository: "example/repo".to_string(),
            api_base_url: base_url,
        };
        let release = source.check_latest().unwrap().unwrap();
        assert_eq!(release.download_url, "https://github.com/example/repo/releases/tag/v1.4.0");
    }

    #[test]
    fn a_repository_with_no_releases_yet_is_not_an_error() {
        let base_url = serve_once("404 Not Found", "{\"message\": \"Not Found\"}");
        let source = GitHubReleaseSource {
            repository: "example/brand-new-repo".to_string(),
            api_base_url: base_url,
        };
        assert!(source.check_latest().unwrap().is_none());
    }
}

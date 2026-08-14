use crate::dat::parse_dat_file;
use crate::model::GameSet;
use retrotools_common::error::{AppError, AppResult};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DatSource {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct DatUpdateReport {
    pub name: String,
    pub previous_version: Option<String>,
    pub new_version: String,
    pub changed: bool,
    pub file_path: PathBuf,
    pub gameset: GameSet,
}

fn sanitize_component(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c => c,
        })
        .collect();
    cleaned.trim().trim_end_matches('.').to_string()
}

fn guess_extension(url: &str, content: &[u8]) -> &'static str {
    if content.len() >= 2 && content[0..2] == [0x50, 0x4B] {
        return "zip";
    }
    if url.to_lowercase().ends_with(".zip") {
        return "zip";
    }
    "dat"
}

/// Whether `gameset`'s DAT version differs from `previous_version` (the
/// version the caller already has on file, typically from `DatCache`).
/// A `None` previous version always counts as a change (first import).
fn version_changed(previous_version: Option<&str>, gameset: &GameSet) -> bool {
    previous_version != Some(gameset.dat_version.as_str())
}

/// Downloads `source.url` and saves it under `download_dir` (created if
/// needed), naming the file after the source and its detected content type.
///
/// This is a plain blocking HTTPS GET (`ureq`, backed by `rustls`) with no
/// authentication, cookies or JavaScript execution — it only works against a
/// direct DAT/ZIP download link, not a page that requires logging in or
/// clicking through a "Download" button. No-Intro/Redump/TOSEC don't publish
/// a documented, unauthenticated API for automatic per-platform DAT
/// discovery, so "automatic update" here means: the user supplies the direct
/// URL once (e.g. copied from their browser's download link), and this
/// re-fetches it on demand or on a schedule the host application controls.
pub fn download_dat(source: &DatSource, download_dir: &Path) -> AppResult<PathBuf> {
    let response = ureq::get(&source.url)
        .call()
        .map_err(|e| AppError::DatParsing(format!("cannot download '{}': {e}", source.url)))?;

    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(AppError::Io)?;

    if bytes.is_empty() {
        return Err(AppError::DatParsing(format!(
            "downloaded 0 bytes from '{}'",
            source.url
        )));
    }

    std::fs::create_dir_all(download_dir).map_err(AppError::Io)?;
    let extension = guess_extension(&source.url, &bytes);
    let file_path = download_dir.join(format!("{}.{extension}", sanitize_component(&source.name)));
    std::fs::write(&file_path, &bytes).map_err(AppError::Io)?;

    Ok(file_path)
}

/// Downloads and parses `source`, comparing the freshly fetched DAT version
/// against `previous_version` (typically whatever's already in the local
/// `DatCache` for this platform) so the caller can tell whether anything
/// actually changed before re-importing it.
pub fn check_for_update(
    source: &DatSource,
    download_dir: &Path,
    previous_version: Option<&str>,
) -> AppResult<DatUpdateReport> {
    let file_path = download_dat(source, download_dir)?;
    let gameset = parse_dat_file(&file_path)?;
    let changed = version_changed(previous_version, &gameset);

    Ok(DatUpdateReport {
        name: source.name.clone(),
        previous_version: previous_version.map(|s| s.to_string()),
        new_version: gameset.dat_version.clone(),
        changed,
        file_path,
        gameset,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dat::parse_dat_str;

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<datafile>
  <header><name>Test</name><version>20260101</version></header>
  <game name="Game A"><rom name="a.bin" size="1" crc="00000001"/></game>
</datafile>"#;

    #[test]
    fn sanitizes_unsafe_filename_characters() {
        assert_eq!(
            sanitize_component("No-Intro: Game Boy?"),
            "No-Intro_ Game Boy_"
        );
    }

    #[test]
    fn guesses_zip_from_magic_bytes_over_url() {
        let zip_magic = [0x50, 0x4B, 0x03, 0x04];
        assert_eq!(
            guess_extension("https://example.com/dat.download", &zip_magic),
            "zip"
        );
    }

    #[test]
    fn guesses_zip_from_url_extension_when_content_is_ambiguous() {
        assert_eq!(
            guess_extension("https://example.com/pack.zip", b"<?xml"),
            "zip"
        );
    }

    #[test]
    fn falls_back_to_dat_extension() {
        assert_eq!(guess_extension("https://example.com/pack", b"<?xml"), "dat");
    }

    #[test]
    fn detects_a_changed_version() {
        let gameset = parse_dat_str(SAMPLE, "Test").unwrap();
        assert!(version_changed(Some("20250101"), &gameset));
        assert!(version_changed(None, &gameset));
        assert!(!version_changed(Some("20260101"), &gameset));
    }
}

use crate::external_tools::{find_tool, ExternalTool};
use crate::hash::{compute_hashes, HashResult};
use retrotools_common::error::{AppError, AppResult};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArchiveKind {
    Zip,
    SevenZip,
    Tar,
    Rar,
    Chd,
    None,
}

#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub name: String,
    pub size: u64,
}

/// Detects the archive format from magic bytes (falling back to extension for
/// formats such as TAR that have no reliable leading magic).
pub fn detect_archive_kind(path: &Path) -> AppResult<ArchiveKind> {
    let mut magic = [0u8; 8];
    let mut file = std::fs::File::open(path).map_err(AppError::Io)?;
    let n = file.read(&mut magic).map_err(AppError::Io)?;
    let magic = &magic[..n];

    if magic.len() >= 4
        && (magic[0..4] == [0x50, 0x4B, 0x03, 0x04] || magic[0..4] == [0x50, 0x4B, 0x05, 0x06])
    {
        return Ok(ArchiveKind::Zip);
    }
    if magic.len() >= 6 && magic[0..6] == [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] {
        return Ok(ArchiveKind::SevenZip);
    }
    if magic.len() >= 7 && &magic[0..7] == b"Rar!\x1A\x07\x00" {
        return Ok(ArchiveKind::Rar);
    }
    if magic.len() >= 6 && &magic[0..6] == b"Rar!\x1A\x07" {
        return Ok(ArchiveKind::Rar);
    }
    if magic.len() >= 8 && &magic[0..8] == b"MComprHD" {
        return Ok(ArchiveKind::Chd);
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    if ext.as_deref() == Some("tar") {
        return Ok(ArchiveKind::Tar);
    }

    Ok(ArchiveKind::None)
}

pub fn is_supported_archive(kind: ArchiveKind) -> bool {
    matches!(
        kind,
        ArchiveKind::Zip
            | ArchiveKind::SevenZip
            | ArchiveKind::Tar
            | ArchiveKind::Rar
            | ArchiveKind::Chd
    )
}

fn unsupported_error(kind: ArchiveKind) -> AppError {
    let reason = match kind {
        ArchiveKind::None => "not a recognized archive format",
        _ => "archive format not supported",
    };
    AppError::Scan(reason.to_string())
}

// --- RAR, via the bundled `UnRAR.exe` (read-only extraction is free to
// redistribute; RAR's compressed format itself has no maintained Rust
// decoder) -------------------------------------------------------------

fn rar_list_entries(path: &Path) -> AppResult<Vec<ArchiveEntry>> {
    let exe = find_tool(ExternalTool::UnRar)?;
    let output = Command::new(&exe)
        .arg("lb")
        .arg("--")
        .arg(path)
        .output()
        .map_err(|e| AppError::Scan(format!("cannot run unrar: {e}")))?;
    if !output.status.success() {
        return Err(AppError::Scan(format!(
            "unrar listing of '{}' failed: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|name| ArchiveEntry {
            name: name.replace('\\', "/"),
            // UnRAR's bare listing mode doesn't include sizes; the real
            // size is known once the entry is actually extracted/hashed.
            size: 0,
        })
        .collect())
}

fn rar_extract_entry(path: &Path, entry_name: &str, dest: &mut dyn Write) -> AppResult<u64> {
    let exe = find_tool(ExternalTool::UnRar)?;
    let mut child = Command::new(&exe)
        .arg("p")
        .arg("-inul")
        .arg("--")
        .arg(path)
        .arg(entry_name)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Scan(format!("cannot run unrar: {e}")))?;

    let mut stdout = child.stdout.take().expect("stdout was piped");
    let written = std::io::copy(&mut stdout, dest).map_err(AppError::Io)?;
    let output = child.wait_with_output().map_err(AppError::Io)?;
    if !output.status.success() {
        return Err(AppError::Scan(format!(
            "unrar extraction of '{entry_name}' failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(written)
}

// --- CHD, via the bundled `chdman.exe` (MAME's own tool; CHD's hunk
// compression has no Rust implementation) -------------------------------

static CHD_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A CHD is a single compressed disk image, not a container of named
/// entries like a ZIP — so unlike the other formats, every "list"/"hash"/
/// "extract" call here has to actually run `chdman` to materialize it as a
/// plain file first, then expose that one file under a synthetic entry name
/// (`<chd stem>.bin`). Extraction is attempted as a CD image first (the
/// common case for retrogaming CHDs — PS1/Saturn/PC Engine CD/Dreamcast),
/// falling back to a raw/hard-disk extraction otherwise.
fn chd_extract_to_temp(path: &Path) -> AppResult<PathBuf> {
    let exe = find_tool(ExternalTool::Chdman)?;
    let counter = CHD_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_dir = std::env::temp_dir().join(format!("rt26-chd-{}-{counter}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).map_err(AppError::Io)?;

    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());
    let out_bin = temp_dir.join(format!("{stem}.bin"));
    let out_cue = temp_dir.join(format!("{stem}.cue"));

    let cd_result = Command::new(&exe)
        .arg("extractcd")
        .arg("-i")
        .arg(path)
        .arg("-o")
        .arg(&out_cue)
        .arg("-ob")
        .arg(&out_bin)
        .arg("-f")
        .output();
    if matches!(&cd_result, Ok(output) if output.status.success()) && out_bin.is_file() {
        return Ok(out_bin);
    }

    let raw_output = Command::new(&exe)
        .arg("extractraw")
        .arg("-i")
        .arg(path)
        .arg("-o")
        .arg(&out_bin)
        .arg("-f")
        .output()
        .map_err(|e| AppError::Scan(format!("cannot run chdman: {e}")))?;
    if !raw_output.status.success() || !out_bin.is_file() {
        std::fs::remove_dir_all(&temp_dir).ok();
        return Err(AppError::Scan(format!(
            "chdman extraction of '{}' failed: {}",
            path.display(),
            String::from_utf8_lossy(&raw_output.stderr).trim()
        )));
    }
    Ok(out_bin)
}

fn chd_entry_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());
    format!("{stem}.bin")
}

fn chd_list_entries(path: &Path) -> AppResult<Vec<ArchiveEntry>> {
    let extracted = chd_extract_to_temp(path)?;
    let size = std::fs::metadata(&extracted).map(|m| m.len()).unwrap_or(0);
    let name = extracted
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if let Some(parent) = extracted.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
    Ok(vec![ArchiveEntry { name, size }])
}

fn chd_hash_entry(path: &Path, entry_name: &str) -> AppResult<HashResult> {
    if entry_name != chd_entry_name(path) {
        return Err(AppError::Scan(format!(
            "entry '{entry_name}' not found in CHD image"
        )));
    }
    let extracted = chd_extract_to_temp(path)?;
    let file = std::fs::File::open(&extracted).map_err(AppError::Io)?;
    let result = compute_hashes(file);
    if let Some(parent) = extracted.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
    result
}

fn chd_extract_entry(path: &Path, entry_name: &str, dest: &mut dyn Write) -> AppResult<u64> {
    if entry_name != chd_entry_name(path) {
        return Err(AppError::Scan(format!(
            "entry '{entry_name}' not found in CHD image"
        )));
    }
    let extracted = chd_extract_to_temp(path)?;
    let result = (|| {
        let mut file = std::fs::File::open(&extracted).map_err(AppError::Io)?;
        std::io::copy(&mut file, dest).map_err(AppError::Io)
    })();
    if let Some(parent) = extracted.parent() {
        std::fs::remove_dir_all(parent).ok();
    }
    result
}

pub fn list_entries(path: &Path, kind: ArchiveKind) -> AppResult<Vec<ArchiveEntry>> {
    match kind {
        ArchiveKind::Zip => {
            let file = std::fs::File::open(path).map_err(AppError::Io)?;
            let mut archive = zip::ZipArchive::new(file)
                .map_err(|e| AppError::Scan(format!("invalid ZIP archive: {e}")))?;
            let mut entries = Vec::with_capacity(archive.len());
            for i in 0..archive.len() {
                let entry = archive
                    .by_index(i)
                    .map_err(|e| AppError::Scan(format!("cannot read ZIP entry {i}: {e}")))?;
                if entry.is_file() {
                    entries.push(ArchiveEntry {
                        name: entry.name().to_string(),
                        size: entry.size(),
                    });
                }
            }
            Ok(entries)
        }
        ArchiveKind::Tar => {
            let file = std::fs::File::open(path).map_err(AppError::Io)?;
            let mut archive = tar::Archive::new(file);
            let mut entries = Vec::new();
            for entry in archive.entries().map_err(AppError::Io)? {
                let entry = entry.map_err(AppError::Io)?;
                if entry.header().entry_type().is_file() {
                    entries.push(ArchiveEntry {
                        name: entry
                            .path()
                            .map_err(AppError::Io)?
                            .to_string_lossy()
                            .to_string(),
                        size: entry.header().size().unwrap_or(0),
                    });
                }
            }
            Ok(entries)
        }
        ArchiveKind::SevenZip => {
            let archive = sevenz_rust::Archive::open(path)
                .map_err(|e| AppError::Scan(format!("invalid 7Z archive: {e}")))?;
            Ok(archive
                .files
                .iter()
                .filter(|f| f.has_stream && !f.is_directory)
                .map(|f| ArchiveEntry {
                    name: f.name.clone(),
                    size: f.size,
                })
                .collect())
        }
        ArchiveKind::Rar => rar_list_entries(path),
        ArchiveKind::Chd => chd_list_entries(path),
        other => Err(unsupported_error(other)),
    }
}

pub fn hash_entry(path: &Path, kind: ArchiveKind, entry_name: &str) -> AppResult<HashResult> {
    match kind {
        ArchiveKind::Zip => {
            let file = std::fs::File::open(path).map_err(AppError::Io)?;
            let mut archive = zip::ZipArchive::new(file)
                .map_err(|e| AppError::Scan(format!("invalid ZIP archive: {e}")))?;
            let entry = archive.by_name(entry_name).map_err(|e| {
                AppError::Scan(format!("cannot read ZIP entry '{entry_name}': {e}"))
            })?;
            compute_hashes(entry)
        }
        ArchiveKind::Tar => {
            let file = std::fs::File::open(path).map_err(AppError::Io)?;
            let mut archive = tar::Archive::new(file);
            for entry in archive.entries().map_err(AppError::Io)? {
                let entry = entry.map_err(AppError::Io)?;
                let name = entry
                    .path()
                    .map_err(AppError::Io)?
                    .to_string_lossy()
                    .to_string();
                if name == entry_name {
                    return compute_hashes(entry);
                }
            }
            Err(AppError::Scan(format!(
                "entry '{entry_name}' not found in TAR archive"
            )))
        }
        ArchiveKind::SevenZip => {
            let mut result = None;
            sevenz_rust::SevenZReader::open(path, sevenz_rust::Password::empty())
                .map_err(|e| AppError::Scan(format!("invalid 7Z archive: {e}")))?
                .for_each_entries(|entry, reader| {
                    if entry.name == entry_name {
                        let mut buf = Vec::with_capacity(entry.size as usize);
                        reader
                            .read_to_end(&mut buf)
                            .map_err(sevenz_rust::Error::io)?;
                        result = Some(buf);
                        return Ok(false);
                    }
                    Ok(true)
                })
                .map_err(|e| AppError::Scan(format!("cannot read 7Z entry '{entry_name}': {e}")))?;
            let buf = result.ok_or_else(|| {
                AppError::Scan(format!("entry '{entry_name}' not found in 7Z archive"))
            })?;
            compute_hashes(buf.as_slice())
        }
        ArchiveKind::Rar => {
            let mut buf = Vec::new();
            rar_extract_entry(path, entry_name, &mut buf)?;
            compute_hashes(buf.as_slice())
        }
        ArchiveKind::Chd => chd_hash_entry(path, entry_name),
        other => Err(unsupported_error(other)),
    }
}

/// Streams a single archive entry's decoded bytes into `dest`, without
/// extracting anything else in the archive. Returns the number of bytes
/// written.
pub fn extract_entry(
    path: &Path,
    kind: ArchiveKind,
    entry_name: &str,
    dest: &mut dyn std::io::Write,
) -> AppResult<u64> {
    match kind {
        ArchiveKind::Zip => {
            let file = std::fs::File::open(path).map_err(AppError::Io)?;
            let mut archive = zip::ZipArchive::new(file)
                .map_err(|e| AppError::Scan(format!("invalid ZIP archive: {e}")))?;
            let mut entry = archive.by_name(entry_name).map_err(|e| {
                AppError::Scan(format!("cannot read ZIP entry '{entry_name}': {e}"))
            })?;
            std::io::copy(&mut entry, dest).map_err(AppError::Io)
        }
        ArchiveKind::Tar => {
            let file = std::fs::File::open(path).map_err(AppError::Io)?;
            let mut archive = tar::Archive::new(file);
            for entry in archive.entries().map_err(AppError::Io)? {
                let mut entry = entry.map_err(AppError::Io)?;
                let name = entry
                    .path()
                    .map_err(AppError::Io)?
                    .to_string_lossy()
                    .to_string();
                if name == entry_name {
                    return std::io::copy(&mut entry, dest).map_err(AppError::Io);
                }
            }
            Err(AppError::Scan(format!(
                "entry '{entry_name}' not found in TAR archive"
            )))
        }
        ArchiveKind::SevenZip => {
            let mut written = 0u64;
            let mut found = false;
            sevenz_rust::SevenZReader::open(path, sevenz_rust::Password::empty())
                .map_err(|e| AppError::Scan(format!("invalid 7Z archive: {e}")))?
                .for_each_entries(|entry, reader| {
                    if entry.name == entry_name {
                        written = std::io::copy(reader, dest).map_err(sevenz_rust::Error::io)?;
                        found = true;
                        return Ok(false);
                    }
                    Ok(true)
                })
                .map_err(|e| AppError::Scan(format!("cannot read 7Z entry '{entry_name}': {e}")))?;
            if !found {
                return Err(AppError::Scan(format!(
                    "entry '{entry_name}' not found in 7Z archive"
                )));
            }
            Ok(written)
        }
        ArchiveKind::Rar => rar_extract_entry(path, entry_name, dest),
        ArchiveKind::Chd => chd_extract_entry(path, entry_name, dest),
        other => Err(unsupported_error(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_and_reads_zip_archive() {
        let dir = std::env::temp_dir().join(format!("rt26-archive-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let zip_path = dir.join("sample.zip");
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("rom.bin", zip::write::FileOptions::default())
            .unwrap();
        writer.write_all(b"rom payload").unwrap();
        writer.finish().unwrap();

        let kind = detect_archive_kind(&zip_path).unwrap();
        assert_eq!(kind, ArchiveKind::Zip);

        let entries = list_entries(&zip_path, kind).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "rom.bin");

        let result = hash_entry(&zip_path, kind, "rom.bin").unwrap();
        assert_eq!(result.full.size, 11);

        let mut extracted = Vec::new();
        let written = extract_entry(&zip_path, kind, "rom.bin", &mut extracted).unwrap();
        assert_eq!(written, 11);
        assert_eq!(extracted, b"rom payload");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn detects_plain_file_as_none() {
        let dir =
            std::env::temp_dir().join(format!("rt26-archive-test-plain-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("plain.bin");
        std::fs::write(&path, b"not an archive").unwrap();
        assert_eq!(detect_archive_kind(&path).unwrap(), ArchiveKind::None);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Builds a real CHD from a random payload via the bundled `chdman.exe`
    /// and round-trips it through `list_entries`/`hash_entry`/`extract_entry`.
    /// Skips (rather than fails) on a machine that doesn't have
    /// `resources/chdman.exe` available, since that's a legitimate state for
    /// anyone building this repo without the bundled tools.
    #[test]
    fn extracts_a_real_chd_round_trip() {
        let Ok(chdman) = find_tool(ExternalTool::Chdman) else {
            eprintln!("skipping: chdman.exe not found");
            return;
        };

        let dir =
            std::env::temp_dir().join(format!("rt26-chd-archive-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input_path = dir.join("input.bin");
        let payload: Vec<u8> = (0..65536u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&input_path, &payload).unwrap();

        let chd_path = dir.join("test.chd");
        let create_output = Command::new(&chdman)
            .arg("createraw")
            .arg("-i")
            .arg(&input_path)
            .arg("-o")
            .arg(&chd_path)
            .arg("-us")
            .arg("512")
            .output()
            .unwrap();
        assert!(
            create_output.status.success(),
            "chdman createraw failed: {}",
            String::from_utf8_lossy(&create_output.stderr)
        );

        let kind = detect_archive_kind(&chd_path).unwrap();
        assert_eq!(kind, ArchiveKind::Chd);

        let entries = list_entries(&chd_path, kind).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "test.bin");
        assert_eq!(entries[0].size, payload.len() as u64);

        let hash_result = hash_entry(&chd_path, kind, "test.bin").unwrap();
        assert_eq!(hash_result.full.size, payload.len() as u64);

        let mut extracted = Vec::new();
        let written = extract_entry(&chd_path, kind, "test.bin", &mut extracted).unwrap();
        assert_eq!(written, payload.len() as u64);
        assert_eq!(extracted, payload);

        std::fs::remove_dir_all(&dir).ok();
    }

    /// UnRAR can only extract, not create, archives, so there is no way to
    /// fabricate a real, valid `.rar` fixture from this test suite. This
    /// instead checks that the tool integration doesn't panic or hang
    /// against a file carrying the real RAR magic bytes (so it passes our
    /// own `detect_archive_kind`, matching what actually reaches this code
    /// path in `scan.rs`) but truncated/corrupt content — UnRAR reports
    /// that as an empty listing rather than a non-zero exit, so we only
    /// assert the call completes cleanly, not that it errors.
    #[test]
    fn rar_listing_handles_a_corrupt_archive_without_panicking() {
        if find_tool(ExternalTool::UnRar).is_err() {
            eprintln!("skipping: UnRAR.exe not found");
            return;
        }

        let dir =
            std::env::temp_dir().join(format!("rt26-rar-archive-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake_rar = dir.join("corrupt.rar");
        let mut content = b"Rar!\x1A\x07\x00".to_vec();
        content.extend_from_slice(b"not actually valid RAR data after the magic bytes");
        std::fs::write(&fake_rar, &content).unwrap();

        assert_eq!(detect_archive_kind(&fake_rar).unwrap(), ArchiveKind::Rar);
        let result = rar_list_entries(&fake_rar);
        // Either an explicit error or an empty listing is acceptable; a
        // panic or hang is not (the assert above already exercised the call).
        if let Ok(entries) = result {
            assert!(entries.is_empty());
        }

        std::fs::remove_dir_all(&dir).ok();
    }
}

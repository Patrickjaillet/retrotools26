use crate::archive::{self, ArchiveKind};
use crate::cache::ScanCache;
use crate::hash::{self, FileHashes, HashResult};
use crate::header::RomHeaderKind;
use retrotools_common::error::AppResult;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::SystemTime;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub roots: Vec<PathBuf>,
    pub recursive: bool,
    pub scan_inside_archives: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            recursive: true,
            scan_inside_archives: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScannedRom {
    pub platform_hint: String,
    pub source_path: PathBuf,
    pub archive_entry: Option<String>,
    pub file_name: String,
    pub hashes: FileHashes,
    pub headerless_hashes: Option<FileHashes>,
    pub header_kind: RomHeaderKind,
}

#[derive(Debug, Clone)]
pub struct ScanErrorEntry {
    pub source_path: PathBuf,
    pub archive_entry: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct ScanProgress {
    pub files_scanned: usize,
    pub files_total: usize,
    pub bytes_scanned: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ScanOutcome {
    pub roms: Vec<ScannedRom>,
    pub errors: Vec<ScanErrorEntry>,
}

struct CandidateFile {
    path: PathBuf,
    platform_hint: String,
    archive_kind: ArchiveKind,
}

/// Each scan root is treated as one platform (e.g. a "Nintendo - Game Boy"
/// folder); files found anywhere below it, however deep, share that hint.
fn platform_hint_for(root: &Path) -> String {
    root.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn collect_candidates(options: &ScanOptions) -> AppResult<Vec<CandidateFile>> {
    let mut candidates = Vec::new();
    for root in &options.roots {
        let max_depth = if options.recursive { usize::MAX } else { 1 };
        for entry in WalkDir::new(root)
            .max_depth(max_depth)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path().to_path_buf();
            let archive_kind = archive::detect_archive_kind(&path).unwrap_or(ArchiveKind::None);
            candidates.push(CandidateFile {
                platform_hint: platform_hint_for(root),
                path,
                archive_kind,
            });
        }
    }
    Ok(candidates)
}

fn file_mtime_secs(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn hash_with_cache(
    cache: Option<&ScanCache>,
    path: &Path,
    archive_entry: Option<&str>,
    mtime: i64,
    known_size: Option<u64>,
    compute: impl FnOnce() -> AppResult<HashResult>,
) -> AppResult<HashResult> {
    let path_str = path.to_string_lossy().to_string();
    if let (Some(cache), Some(size)) = (cache, known_size) {
        if let Some(cached) = cache.get(&path_str, archive_entry, mtime, size)? {
            return Ok(cached);
        }
    }

    let result = compute()?;

    if let Some(cache) = cache {
        let _ = cache.put(&path_str, archive_entry, mtime, result.full.size, &result);
    }

    Ok(result)
}

/// Scans one or more ROM directories, computing CRC32/MD5/SHA1/SHA256 hashes
/// for every file found (streaming into supported archives), in parallel via
/// rayon. `cache` allows skipping re-hashing of files that have not changed
/// since the last scan (matched on path + mtime + size). `on_progress` is
/// invoked after each file/entry completes.
pub fn scan(
    options: &ScanOptions,
    cache: Option<&ScanCache>,
    on_progress: Option<&(dyn Fn(ScanProgress) + Send + Sync)>,
) -> AppResult<ScanOutcome> {
    let candidates = collect_candidates(options)?;

    struct Unit {
        path: PathBuf,
        platform_hint: String,
        archive_entry: Option<String>,
        archive_kind: ArchiveKind,
        known_size: Option<u64>,
    }

    let mut units = Vec::new();
    let mut errors = Vec::new();

    for candidate in candidates {
        if candidate.archive_kind != ArchiveKind::None && options.scan_inside_archives {
            if !archive::is_supported_archive(candidate.archive_kind) {
                errors.push(ScanErrorEntry {
                    source_path: candidate.path.clone(),
                    archive_entry: None,
                    message: format!(
                        "{:?} archives are not yet supported for scanning",
                        candidate.archive_kind
                    ),
                });
                continue;
            }
            match archive::list_entries(&candidate.path, candidate.archive_kind) {
                Ok(entries) => {
                    for entry in entries {
                        units.push(Unit {
                            path: candidate.path.clone(),
                            platform_hint: candidate.platform_hint.clone(),
                            archive_entry: Some(entry.name),
                            archive_kind: candidate.archive_kind,
                            known_size: Some(entry.size),
                        });
                    }
                }
                Err(err) => errors.push(ScanErrorEntry {
                    source_path: candidate.path.clone(),
                    archive_entry: None,
                    message: err.to_string(),
                }),
            }
        } else {
            let known_size = std::fs::metadata(&candidate.path).ok().map(|m| m.len());
            units.push(Unit {
                path: candidate.path,
                platform_hint: candidate.platform_hint,
                archive_entry: None,
                archive_kind: ArchiveKind::None,
                known_size,
            });
        }
    }

    let total = units.len();
    let scanned_counter = Arc::new(AtomicUsize::new(0));
    let bytes_counter = Arc::new(AtomicU64::new(0));

    let results: Vec<Result<ScannedRom, ScanErrorEntry>> = units
        .into_par_iter()
        .map(|unit| {
            let mtime = file_mtime_secs(&unit.path);
            let archive_entry_ref = unit.archive_entry.as_deref();

            let hash_result = hash_with_cache(
                cache,
                &unit.path,
                archive_entry_ref,
                mtime,
                unit.known_size,
                || match archive_entry_ref {
                    Some(entry_name) => {
                        archive::hash_entry(&unit.path, unit.archive_kind, entry_name)
                    }
                    None => hash::compute_hashes_for_file(&unit.path),
                },
            );

            let scanned = scanned_counter.fetch_add(1, Ordering::Relaxed) + 1;

            let result = match hash_result {
                Ok(hr) => {
                    bytes_counter.fetch_add(hr.full.size, Ordering::Relaxed);
                    let file_name = unit
                        .archive_entry
                        .clone()
                        .unwrap_or_else(|| {
                            unit.path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default()
                        });
                    Ok(ScannedRom {
                        platform_hint: unit.platform_hint,
                        source_path: unit.path,
                        archive_entry: unit.archive_entry,
                        file_name,
                        hashes: hr.full,
                        headerless_hashes: hr.headerless,
                        header_kind: hr.header.kind,
                    })
                }
                Err(err) => Err(ScanErrorEntry {
                    source_path: unit.path,
                    archive_entry: unit.archive_entry,
                    message: err.to_string(),
                }),
            };

            if let Some(callback) = on_progress {
                callback(ScanProgress {
                    files_scanned: scanned,
                    files_total: total,
                    bytes_scanned: bytes_counter.load(Ordering::Relaxed),
                });
            }

            result
        })
        .collect();

    let mut roms = Vec::with_capacity(results.len());
    for result in results {
        match result {
            Ok(rom) => roms.push(rom),
            Err(err) => errors.push(err),
        }
    }

    Ok(ScanOutcome { roms, errors })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rt26-scan-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn scans_plain_files_recursively() {
        let root = temp_dir("plain");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("a.bin"), b"content-a").unwrap();
        std::fs::write(root.join("sub").join("b.bin"), b"content-b").unwrap();

        let options = ScanOptions {
            roots: vec![root.clone()],
            recursive: true,
            scan_inside_archives: true,
        };
        let outcome = scan(&options, None, None).unwrap();
        assert_eq!(outcome.roms.len(), 2);
        assert!(outcome.errors.is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn scans_inside_zip_archives() {
        let root = temp_dir("zip");
        let zip_path = root.join("game.zip");
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("game.bin", zip::write::FileOptions::default())
            .unwrap();
        writer.write_all(b"zipped-rom").unwrap();
        writer.finish().unwrap();

        let options = ScanOptions {
            roots: vec![root.clone()],
            recursive: true,
            scan_inside_archives: true,
        };
        let outcome = scan(&options, None, None).unwrap();
        assert_eq!(outcome.roms.len(), 1);
        assert_eq!(outcome.roms[0].archive_entry.as_deref(), Some("game.bin"));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn reuses_cached_hash_for_unchanged_file() {
        let root = temp_dir("cache");
        let file_path = root.join("cached.bin");
        std::fs::write(&file_path, b"cache-me").unwrap();

        let cache = ScanCache::open_in_memory().unwrap();
        let options = ScanOptions {
            roots: vec![root.clone()],
            recursive: true,
            scan_inside_archives: true,
        };

        let first = scan(&options, Some(&cache), None).unwrap();
        assert_eq!(first.roms.len(), 1);
        let first_hash = first.roms[0].hashes.clone();

        let second = scan(&options, Some(&cache), None).unwrap();
        assert_eq!(second.roms[0].hashes, first_hash);

        std::fs::remove_dir_all(&root).ok();
    }
}

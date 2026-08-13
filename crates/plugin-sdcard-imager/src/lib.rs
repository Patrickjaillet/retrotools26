use retrotools_plugin_api::{Plugin, PluginContext, PluginOutcome, PluginResult};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Writing a raw image to a physical disk or USB stick is destructive and
/// irreversible — everything already on the device is gone the moment the
/// write starts. Every function in this module that touches a real target
/// is designed around that: checksum verification is a separate, safe
/// first step; device writing requires an explicit typed confirmation of
/// the exact device id (the same "type the drive name back" pattern
/// balenaEtcher/Raspberry Pi Imager use); and `write_image` takes a plain
/// `Path` rather than a `RemovableDevice` so it can be exercised in tests
/// against a throwaway file instead of real hardware. See
/// `docs/PLUGIN_DEV.md` for the manual procedure to validate against an
/// actual SD card/USB stick — that step is not, and should not be,
/// automated in CI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumAlgorithm {
    Sha256,
    Md5,
}

/// Verifies a downloaded base image (Batocera/Recalbox/Lakka, etc.) against
/// a checksum the user copied from the distribution's official release
/// page. Reuses `retrotools_core::hash` (the same hashing already used for
/// ROM scanning) rather than reimplementing it.
pub fn verify_checksum(image_path: &Path, algorithm: ChecksumAlgorithm, expected_hex: &str) -> Result<(), String> {
    if !image_path.is_file() {
        return Err(format!("'{}' is not a file", image_path.display()));
    }
    let hashes = retrotools_core::hash::compute_hashes_for_file(image_path).map_err(|e| e.to_string())?;
    let actual = match algorithm {
        ChecksumAlgorithm::Sha256 => &hashes.full.sha256,
        ChecksumAlgorithm::Md5 => &hashes.full.md5,
    };
    let expected = expected_hex.trim().to_lowercase();
    if actual.to_lowercase() != expected {
        return Err(format!("checksum mismatch: expected {expected}, got {actual}"));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovableDevice {
    pub id: String,
    pub model: String,
    pub size_bytes: u64,
}

fn parse_devices(stdout: &str) -> Vec<RemovableDevice> {
    stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '|');
            let id = parts.next()?.trim().to_string();
            let model = parts.next()?.trim().to_string();
            let size_bytes: u64 = parts.next()?.trim().parse().ok()?;
            if id.is_empty() {
                return None;
            }
            Some(RemovableDevice { id, model, size_bytes })
        })
        .collect()
}

/// Enumerates removable (USB) disks via PowerShell's WMI/CIM interface.
/// Returns an empty list rather than an error when PowerShell can't be run
/// or finds nothing — "couldn't check" and "nothing found" both mean
/// "nothing to pick from" to a caller building a device list, they aren't
/// failure conditions worth surfacing differently here.
pub fn list_removable_devices() -> Vec<RemovableDevice> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-CimInstance Win32_DiskDrive | Where-Object { $_.InterfaceType -eq 'USB' } | ForEach-Object { \"$($_.DeviceID)|$($_.Model)|$($_.Size)\" }",
        ])
        .output();
    match output {
        Ok(output) if output.status.success() => parse_devices(&String::from_utf8_lossy(&output.stdout)),
        _ => Vec::new(),
    }
}

pub type WriteLog = Vec<String>;

/// Writes `image_path` to `destination` byte-for-byte. Refuses unless
/// `confirm_token` exactly equals `device_id_to_confirm` — the caller is
/// expected to have the user type/select the target device id a second
/// time, so a stale selection or a copy-paste mistake can't silently wipe
/// the wrong disk. `dry_run` performs every check without opening
/// `destination` for writing at all.
pub fn write_image(image_path: &Path, destination: &Path, device_id_to_confirm: &str, confirm_token: &str, dry_run: bool) -> Result<WriteLog, String> {
    let mut log = Vec::new();
    if !image_path.is_file() {
        return Err(format!("'{}' is not a file", image_path.display()));
    }
    if confirm_token != device_id_to_confirm {
        return Err("confirmation does not match the target device id — nothing was written".into());
    }
    log.push(format!("confirmed target: {device_id_to_confirm}"));

    let size = std::fs::metadata(image_path).map_err(|e| e.to_string())?.len();
    log.push(format!("image size: {size} bytes"));

    if dry_run {
        log.push(format!("[dry run] would write '{}' to '{}'", image_path.display(), destination.display()));
        return Ok(log);
    }

    let mut src = std::fs::File::open(image_path).map_err(|e| e.to_string())?;
    // A real device has a fixed size and can't be truncated; `truncate(false)`
    // is also correct for the plain-file destination tests use, since the
    // full image is always written from offset 0 regardless.
    let mut dst = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(destination)
        .map_err(|e| format!("cannot open '{}' for writing: {e}", destination.display()))?;
    log.push(format!("opened '{}' for writing", destination.display()));

    let mut buffer = vec![0u8; 1024 * 1024];
    let mut written: u64 = 0;
    loop {
        let n = src.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        dst.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        written += n as u64;
    }
    dst.flush().map_err(|e| e.to_string())?;
    log.push(format!("wrote {written} byte(s)"));
    Ok(log)
}

fn collect_relative_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_path_buf());
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Copies a staging folder onto an already-mounted SD/USB partition.
/// Mounting itself is left to Windows (removable FAT32 media gets a drive
/// letter automatically once the image write above completes) — this
/// plugin only handles the "copy content onto the mounted partition" step,
/// via the normal `PluginContext` `source_dir`/`output_dir` contract used
/// throughout the plugin system. `source_dir` is expected to already
/// contain everything meant for the card: the built 1G1R ROM set, plus
/// whatever the Export (Phase 11), Controllers (Phase 15) and Shaders
/// (Phase 16) plugins wrote if the user staged them into the same folder
/// first — this plugin doesn't know or care which of those produced a
/// given file, it just mirrors the whole staging tree onto the target.
pub struct SdCardInjectPlugin;

impl Plugin for SdCardInjectPlugin {
    fn id(&self) -> &'static str {
        "sdcard-inject"
    }

    fn name(&self) -> &'static str {
        "SD/USB Content Injection"
    }

    fn description(&self) -> &'static str {
        "Copy a staging folder (built 1G1R set plus any Export/Controllers/Shaders output) onto an already-mounted SD/USB partition."
    }

    fn run(&self, ctx: &PluginContext) -> PluginResult<PluginOutcome> {
        let source = ctx.source_dir.ok_or("a source directory is required — point it at the staging folder to copy from")?;
        if !source.is_dir() {
            return Err(format!("'{}' is not a directory", source.display()));
        }
        let files = collect_relative_files(source).map_err(|e| e.to_string())?;
        if files.is_empty() {
            return Err(format!("no files found under '{}'", source.display()));
        }

        if ctx.dry_run {
            return Ok(PluginOutcome {
                summary: format!("[dry run] would copy {} file(s) to '{}'", files.len(), ctx.output_dir.display()),
                files_written: Vec::new(),
            });
        }

        std::fs::create_dir_all(ctx.output_dir).map_err(|e| e.to_string())?;
        let mut files_written = Vec::new();
        for rel in &files {
            let src_path = source.join(rel);
            let dest_path = ctx.output_dir.join(rel);
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::copy(&src_path, &dest_path).map_err(|e| format!("cannot copy '{}': {e}", src_path.display()))?;
            files_written.push(dest_path);
        }

        Ok(PluginOutcome {
            summary: format!("copied {} file(s) to '{}'", files_written.len(), ctx.output_dir.display()),
            files_written,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retrotools_core::{DatHeader, DatType, GameSet};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rt26-plugin-sdcard-imager-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn empty_gameset() -> GameSet {
        GameSet {
            platform: "Test".into(),
            dat_name: "Test".into(),
            dat_version: "1".into(),
            dat_type: DatType::Custom,
            header: DatHeader::default(),
            games: Vec::new(),
        }
    }

    #[test]
    fn verify_checksum_accepts_a_matching_sha256() {
        let dir = temp_dir("checksum-ok");
        let image = dir.join("base.img");
        std::fs::write(&image, b"hello retro world").unwrap();
        let expected = retrotools_core::hash::compute_hashes_for_file(&image).unwrap().full.sha256;
        assert!(verify_checksum(&image, ChecksumAlgorithm::Sha256, &expected).is_ok());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn verify_checksum_rejects_a_mismatch() {
        let dir = temp_dir("checksum-bad");
        let image = dir.join("base.img");
        std::fs::write(&image, b"hello retro world").unwrap();
        let err = verify_checksum(&image, ChecksumAlgorithm::Sha256, "0000000000000000000000000000000000000000000000000000000000000000").unwrap_err();
        assert!(err.contains("mismatch"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parse_devices_reads_the_powershell_pipe_delimited_format() {
        let stdout = "\\\\.\\PHYSICALDRIVE1|SanDisk Ultra USB|31000000000\n\\\\.\\PHYSICALDRIVE2|Generic Flash Disk|8000000000\n";
        let devices = parse_devices(stdout);
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].model, "SanDisk Ultra USB");
        assert_eq!(devices[1].size_bytes, 8_000_000_000);
    }

    #[test]
    fn write_image_refuses_a_mismatched_confirmation() {
        let dir = temp_dir("write-mismatch");
        let image = dir.join("base.img");
        std::fs::write(&image, vec![0xAB; 4096]).unwrap();
        let dest = dir.join("fake-device.bin");
        let err = write_image(&image, &dest, "\\\\.\\PHYSICALDRIVE9", "\\\\.\\PHYSICALDRIVE1", false).unwrap_err();
        assert!(err.contains("does not match"));
        assert!(!dest.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_image_dry_run_writes_nothing() {
        let dir = temp_dir("write-dry-run");
        let image = dir.join("base.img");
        std::fs::write(&image, vec![0xCD; 4096]).unwrap();
        let dest = dir.join("fake-device.bin");
        let log = write_image(&image, &dest, "DEV1", "DEV1", true).unwrap();
        assert!(log.iter().any(|l| l.starts_with("[dry run]")));
        assert!(!dest.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_image_writes_identical_bytes_to_a_fake_device_file() {
        let dir = temp_dir("write-real");
        let image = dir.join("base.img");
        let payload: Vec<u8> = (0..(3 * 1024 * 1024 + 777)).map(|i| (i % 251) as u8).collect();
        std::fs::write(&image, &payload).unwrap();
        let dest = dir.join("fake-device.bin");
        let log = write_image(&image, &dest, "DEV1", "DEV1", false).unwrap();
        assert!(log.iter().any(|l| l.contains("wrote")));
        let written = std::fs::read(&dest).unwrap();
        assert_eq!(written, payload);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn inject_plugin_requires_a_source_dir() {
        let gs = empty_gameset();
        let output = temp_dir("inject-no-source");
        let ctx = PluginContext {
            gameset: &gs,
            kept_game_names: &[],
            source_dir: None,
            output_dir: &output,
            match_report: None,
            dry_run: false,
        };
        let err = SdCardInjectPlugin.run(&ctx).unwrap_err();
        assert!(err.contains("source directory"));
        std::fs::remove_dir_all(&output).ok();
    }

    #[test]
    fn inject_plugin_mirrors_the_staging_tree() {
        let source = temp_dir("inject-source");
        std::fs::write(source.join("es_systems.cfg"), "<systemList></systemList>").unwrap();
        std::fs::create_dir_all(source.join("roms").join("snes")).unwrap();
        std::fs::write(source.join("roms").join("snes").join("game.sfc"), b"romdata").unwrap();

        let output = temp_dir("inject-output");
        let gs = empty_gameset();
        let ctx = PluginContext {
            gameset: &gs,
            kept_game_names: &[],
            source_dir: Some(&source),
            output_dir: &output,
            match_report: None,
            dry_run: false,
        };
        let outcome = SdCardInjectPlugin.run(&ctx).unwrap();
        assert_eq!(outcome.files_written.len(), 2);
        assert!(output.join("es_systems.cfg").is_file());
        assert!(output.join("roms").join("snes").join("game.sfc").is_file());

        std::fs::remove_dir_all(&source).ok();
        std::fs::remove_dir_all(&output).ok();
    }

    #[test]
    fn inject_plugin_dry_run_writes_nothing() {
        let source = temp_dir("inject-dry-source");
        std::fs::write(source.join("es_systems.cfg"), "<systemList></systemList>").unwrap();
        let output = temp_dir("inject-dry-output");
        let gs = empty_gameset();
        let ctx = PluginContext {
            gameset: &gs,
            kept_game_names: &[],
            source_dir: Some(&source),
            output_dir: &output,
            match_report: None,
            dry_run: true,
        };
        let outcome = SdCardInjectPlugin.run(&ctx).unwrap();
        assert!(outcome.summary.starts_with("[dry run]"));
        assert!(!output.join("es_systems.cfg").exists());

        std::fs::remove_dir_all(&source).ok();
        std::fs::remove_dir_all(&output).ok();
    }
}

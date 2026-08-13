use crate::external_tools::{find_tool, ExternalTool};
use retrotools_common::error::{AppError, AppResult};
use std::path::{Path, PathBuf};
use std::process::Command;

/// ROM/ISO format conversion, via the bundled `chdman.exe` (same tool
/// already used for CHD extraction in `archive.rs`). Only CHD is supported:
/// CSO (PSP) and RVZ (Dolphin/GameCube-Wii) each need their own dedicated
/// codec (`maxcso`, `DolphinTool`) that isn't bundled with this app and
/// isn't a byproduct of any other feature, so converting to/from those
/// formats is out of scope for this pass — `chdman` is the one converter
/// already on hand.
fn run_chdman(args: &[&std::ffi::OsStr]) -> AppResult<()> {
    let exe = find_tool(ExternalTool::Chdman)?;
    let output = Command::new(&exe)
        .args(args)
        .output()
        .map_err(|e| AppError::Scan(format!("cannot run chdman: {e}")))?;
    if !output.status.success() {
        return Err(AppError::Scan(format!(
            "chdman failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

/// Creates a CHD from a raw disk image or CD sheet. `source` is dispatched
/// to chdman's `createcd` (for `.cue`/`.gdi`/`.toc`/`.nrg` sheets — the
/// common CD-based retrogaming case: PS1/Saturn/PC Engine CD/Dreamcast) or
/// `createraw` (for a plain `.iso`/`.bin`/`.img` with no accompanying
/// sheet — hard-disk-style images), based on `source`'s extension.
pub fn convert_to_chd(source: &Path, dest_chd: &Path) -> AppResult<PathBuf> {
    if !source.is_file() {
        return Err(AppError::Scan(format!("'{}' is not a file", source.display())));
    }
    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let is_cd_sheet = matches!(extension.as_str(), "cue" | "gdi" | "toc" | "nrg");
    if let Some(parent) = dest_chd.parent() {
        std::fs::create_dir_all(parent).map_err(AppError::Io)?;
    }
    if is_cd_sheet {
        run_chdman(&[
            "createcd".as_ref(),
            "-i".as_ref(),
            source.as_os_str(),
            "-o".as_ref(),
            dest_chd.as_os_str(),
            "-f".as_ref(),
        ])?;
    } else {
        // `createraw` has no CD sheet to infer a sector size from, so it
        // requires an explicit unit size; 512 bytes is the standard
        // hard-disk-style sector size used for plain raw/ISO images.
        run_chdman(&[
            "createraw".as_ref(),
            "-i".as_ref(),
            source.as_os_str(),
            "-o".as_ref(),
            dest_chd.as_os_str(),
            "-us".as_ref(),
            "512".as_ref(),
            "-f".as_ref(),
        ])?;
    }
    Ok(dest_chd.to_path_buf())
}

/// Extracts a CHD back to a raw image, trying a CD-image extraction first
/// (writing both a `.bin` and a `.cue` alongside it) and falling back to a
/// raw/hard-disk extraction (a single `.bin`, no `.cue`) — the same
/// heuristic `archive.rs` uses when scanning inside a CHD, exposed here as
/// a real output file instead of a throwaway temp copy. Returns the path to
/// the extracted `.bin`.
pub fn convert_from_chd(source_chd: &Path, dest_dir: &Path) -> AppResult<PathBuf> {
    if !source_chd.is_file() {
        return Err(AppError::Scan(format!("'{}' is not a file", source_chd.display())));
    }
    std::fs::create_dir_all(dest_dir).map_err(AppError::Io)?;
    let stem = source_chd
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".to_string());
    let out_bin = dest_dir.join(format!("{stem}.bin"));
    let out_cue = dest_dir.join(format!("{stem}.cue"));

    let cd_result = run_chdman(&[
        "extractcd".as_ref(),
        "-i".as_ref(),
        source_chd.as_os_str(),
        "-o".as_ref(),
        out_cue.as_os_str(),
        "-ob".as_ref(),
        out_bin.as_os_str(),
        "-f".as_ref(),
    ]);
    if cd_result.is_ok() && out_bin.is_file() {
        return Ok(out_bin);
    }

    run_chdman(&[
        "extractraw".as_ref(),
        "-i".as_ref(),
        source_chd.as_os_str(),
        "-o".as_ref(),
        out_bin.as_os_str(),
        "-f".as_ref(),
    ])?;
    if !out_bin.is_file() {
        return Err(AppError::Scan(format!(
            "chdman extraction of '{}' did not produce an output file",
            source_chd.display()
        )));
    }
    Ok(out_bin)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rt26-convert-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Real round trip through the bundled `chdman.exe`: build a raw image,
    /// convert it to CHD, convert that CHD back to raw, and confirm the
    /// bytes match. Skips (doesn't fail) if `chdman.exe` isn't available,
    /// since that's a legitimate state on a machine without `resources/`.
    #[test]
    fn round_trips_a_raw_image_through_chd_and_back() {
        if find_tool(ExternalTool::Chdman).is_err() {
            eprintln!("skipping: chdman.exe not found");
            return;
        }

        let dir = temp_dir("roundtrip");
        let source = dir.join("disk.bin");
        let payload: Vec<u8> = (0..65536u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&source, &payload).unwrap();

        let chd_path = dir.join("disk.chd");
        convert_to_chd(&source, &chd_path).unwrap();
        assert!(chd_path.is_file());

        let extracted_dir = dir.join("extracted");
        let extracted = convert_from_chd(&chd_path, &extracted_dir).unwrap();
        let extracted_bytes = std::fs::read(&extracted).unwrap();
        assert_eq!(extracted_bytes, payload);
    }

    #[test]
    fn errors_cleanly_on_a_missing_source_file() {
        let dir = temp_dir("missing-source");
        let result = convert_to_chd(&dir.join("does_not_exist.bin"), &dir.join("out.chd"));
        assert!(result.is_err());
    }
}

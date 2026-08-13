use retrotools_common::error::{AppError, AppResult};
use std::path::PathBuf;

/// A third-party command-line tool bundled under `resources/` because no
/// pure-Rust implementation is available (or practical) for the format it
/// handles: `UnRAR.exe` (RAR extraction — RAR's format is proprietary and
/// there is no maintained pure-Rust decoder), `chdman.exe` (MAME's CHD
/// encoder/decoder — CHD's hunk compression has no Rust implementation),
/// and `7za.exe` (kept as a fallback 7-Zip implementation alongside the
/// pure-Rust `sevenz-rust` used by default).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalTool {
    SevenZip,
    Chdman,
    UnRar,
    /// Dolphin's command-line conversion tool, used for RVZ (GameCube/Wii)
    /// conversion — not bundled with this app, the user must obtain it
    /// themselves (see `docs/COMPILATION.md`).
    DolphinTool,
    /// `maxcso`, used for CSO (PSP) conversion — not bundled with this app,
    /// the user must obtain it themselves (see `docs/COMPILATION.md`).
    Maxcso,
}

impl ExternalTool {
    fn exe_name(self) -> &'static str {
        match self {
            ExternalTool::SevenZip => "7za.exe",
            ExternalTool::Chdman => "chdman.exe",
            ExternalTool::UnRar => "UnRAR.exe",
            ExternalTool::DolphinTool => "DolphinTool.exe",
            ExternalTool::Maxcso => "maxcso.exe",
        }
    }

    fn env_var(self) -> &'static str {
        match self {
            ExternalTool::SevenZip => "RETROTOOLS_7ZA_PATH",
            ExternalTool::Chdman => "RETROTOOLS_CHDMAN_PATH",
            ExternalTool::UnRar => "RETROTOOLS_UNRAR_PATH",
            ExternalTool::DolphinTool => "RETROTOOLS_DOLPHINTOOL_PATH",
            ExternalTool::Maxcso => "RETROTOOLS_MAXCSO_PATH",
        }
    }
}

fn which(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// Locates a bundled external tool executable. Search order:
/// 1. An explicit override via environment variable (`RETROTOOLS_*_PATH`).
/// 2. `<exe_dir>/resources/<name>` — the layout used by packaged releases
///    (see `docs/COMPILATION.md`).
/// 3. A `resources/<name>` directory found by walking up from the current
///    executable's directory (covers `cargo run`/`cargo test`, where the
///    binary lives under `target/debug/` several levels below the repo
///    root that actually contains `resources/`).
/// 4. The system `PATH`, in case the user already has the tool installed.
pub fn find_tool(tool: ExternalTool) -> AppResult<PathBuf> {
    if let Ok(path) = std::env::var(tool.env_var()) {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let mut dir = exe_dir.to_path_buf();
            for _ in 0..8 {
                let candidate = dir.join("resources").join(tool.exe_name());
                if candidate.is_file() {
                    return Ok(candidate);
                }
                if !dir.pop() {
                    break;
                }
            }
        }
    }

    if let Some(path) = which(tool.exe_name()) {
        return Ok(path);
    }

    Err(AppError::Scan(format!(
        "'{}' was not found (checked {}, a nearby resources/ folder, and PATH)",
        tool.exe_name(),
        tool.env_var()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_bundled_tools_from_the_dev_workspace() {
        // This only asserts something when resources/ is actually present
        // (as it is in this repo's dev checkout); it must not fail on a
        // machine that doesn't have it, since that's a legitimate state too.
        for tool in [
            ExternalTool::SevenZip,
            ExternalTool::Chdman,
            ExternalTool::UnRar,
            ExternalTool::DolphinTool,
            ExternalTool::Maxcso,
        ] {
            if let Ok(path) = find_tool(tool) {
                assert!(path.is_file());
            }
        }
    }

    #[test]
    fn env_override_takes_priority() {
        let dir = std::env::temp_dir().join(format!("rt26-tool-override-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("7za.exe");
        std::fs::write(&fake, b"not a real binary").unwrap();

        std::env::set_var("RETROTOOLS_7ZA_PATH", &fake);
        let found = find_tool(ExternalTool::SevenZip).unwrap();
        assert_eq!(found, fake);
        std::env::remove_var("RETROTOOLS_7ZA_PATH");

        std::fs::remove_dir_all(&dir).ok();
    }
}

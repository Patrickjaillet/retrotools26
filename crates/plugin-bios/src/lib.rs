use retrotools_core::{match_scan, scan, ScanOptions};
use retrotools_plugin_api::{Plugin, PluginContext, PluginOutcome, PluginResult};

/// Verifies local BIOS dumps against a BIOS DAT — the same Logiqx/XML format
/// used for regular ROM DATs, which No-Intro also publishes for BIOS packs.
/// Reusing `retrotools_core::dat`/`scan`/`matcher` here (rather than
/// hardcoding checksums) means the plugin never has to bundle or guess any
/// BIOS hash itself: it trusts whatever DAT the user supplies, exactly like
/// the main 1G1R engine trusts a ROM DAT.
pub struct BiosPlugin;

impl Plugin for BiosPlugin {
    fn id(&self) -> &'static str {
        "bios-manager"
    }

    fn name(&self) -> &'static str {
        "BIOS Manager"
    }

    fn description(&self) -> &'static str {
        "Checks a folder of BIOS files against a BIOS DAT and reports which are present, missing or not matching."
    }

    fn run(&self, ctx: &PluginContext) -> PluginResult<PluginOutcome> {
        let Some(source_dir) = ctx.source_dir else {
            return Err(
                "this plugin requires a source directory containing the BIOS files to verify".to_string(),
            );
        };

        let options = ScanOptions {
            roots: vec![source_dir.to_path_buf()],
            recursive: true,
            scan_inside_archives: true,
        };
        let scan_outcome = scan(&options, None, None).map_err(|e| e.to_string())?;
        let match_report = match_scan(ctx.gameset, &scan_outcome.roms);

        std::fs::create_dir_all(ctx.output_dir).map_err(|e| e.to_string())?;
        let report_path = ctx.output_dir.join("bios_report.txt");

        let mut report = format!(
            "BIOS verification for '{}'\nMatched: {}  Corrupt: {}  Unknown: {}  Missing: {}\n\n",
            ctx.gameset.platform,
            match_report.matched.len(),
            match_report.corrupt.len(),
            match_report.unknown.len(),
            match_report.missing.len()
        );
        for missing in &match_report.missing {
            report.push_str(&format!("MISSING: {} ({})\n", missing.rom_name, missing.game_name));
        }
        for corrupt in &match_report.corrupt {
            report.push_str(&format!(
                "CORRUPT: {} (expected to match '{}')\n",
                corrupt.scanned.file_name,
                corrupt.matched_game.as_deref().unwrap_or("?")
            ));
        }
        std::fs::write(&report_path, &report).map_err(|e| e.to_string())?;

        Ok(PluginOutcome {
            summary: format!(
                "{} matched, {} missing, {} corrupt",
                match_report.matched.len(),
                match_report.missing.len(),
                match_report.corrupt.len()
            ),
            files_written: vec![report_path],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retrotools_core::dat::parse_dat_str;
    use std::path::PathBuf;

    const BIOS_DAT: &str = r#"<?xml version="1.0"?>
<datafile>
  <header><name>Sony PlayStation BIOS</name></header>
  <game name="Sony PlayStation (Europe) BIOS">
    <rom name="scph1002.bin" size="4" crc="deadbeef"/>
  </game>
  <game name="Sony PlayStation (USA) BIOS">
    <rom name="scph1001.bin" size="4" crc="baadf00d"/>
  </game>
</datafile>"#;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rt26-plugin-bios-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reports_missing_bios_files() {
        let gameset = parse_dat_str(BIOS_DAT, "PSX BIOS").unwrap();
        let source_dir = temp_dir("source");
        // Content does not matter for this test: whatever hash it produces,
        // it will not match either DAT entry's CRC, so it lands in "unknown"
        // rather than "matched" — the point is that BOTH DAT roms are absent
        // from disk under their expected names and therefore reported missing.
        std::fs::write(source_dir.join("unrelated.bin"), b"not a real bios").unwrap();

        let output_dir = temp_dir("output");
        let ctx = PluginContext {
            gameset: &gameset,
            kept_game_names: &[],
            source_dir: Some(&source_dir),
            output_dir: &output_dir,
            match_report: None,
            dry_run: false,
        };

        let outcome = BiosPlugin.run(&ctx).unwrap();
        assert_eq!(outcome.files_written.len(), 1);
        let report = std::fs::read_to_string(&outcome.files_written[0]).unwrap();
        assert!(report.contains("MISSING: scph1001.bin"));
        assert!(report.contains("MISSING: scph1002.bin"));

        std::fs::remove_dir_all(&source_dir).ok();
        std::fs::remove_dir_all(&output_dir).ok();
    }

    #[test]
    fn fails_without_a_source_directory() {
        let gameset = parse_dat_str(BIOS_DAT, "PSX BIOS").unwrap();
        let output_dir = temp_dir("no-source");
        let ctx = PluginContext {
            gameset: &gameset,
            kept_game_names: &[],
            source_dir: None,
            output_dir: &output_dir,
            match_report: None,
            dry_run: false,
        };
        assert!(BiosPlugin.run(&ctx).is_err());
        std::fs::remove_dir_all(&output_dir).ok();
    }
}

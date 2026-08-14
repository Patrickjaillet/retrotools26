use retrotools_plugin_api::{Plugin, PluginContext, PluginOutcome, PluginResult};
use std::path::{Path, PathBuf};

/// A parsed RetroArch autoconfig profile: an ordered list of
/// `key = "value"` pairs, kept in the order they were read so
/// re-serializing a profile that came from an existing `.cfg` file is a
/// faithful round trip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerProfile {
    pub entries: Vec<(String, String)>,
}

impl ControllerProfile {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn device_name(&self) -> Option<&str> {
        self.get("input_device")
    }
}

/// Parses RetroArch's autoconfig `.cfg` format: one `key = "value"` (or
/// `key = value`) assignment per line, `#` comments and blank lines
/// ignored. This is the same simple format RetroArch itself reads from
/// `autoconfig/` — no INI sections, no nesting.
pub fn parse_autoconfig(text: &str) -> ControllerProfile {
    let mut entries = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_string();
        let value = value.trim().trim_matches('"').to_string();
        entries.push((key, value));
    }
    ControllerProfile { entries }
}

/// Serializes back to the same `key = "value"` format RetroArch expects.
pub fn serialize_autoconfig(profile: &ControllerProfile) -> String {
    let mut out = String::new();
    for (key, value) in &profile.entries {
        out.push_str(&format!("{key} = \"{value}\"\n"));
    }
    out
}

/// A profile needs at minimum an `input_driver` and `input_device` to
/// identify itself to RetroArch, and at least one real button/axis mapping
/// (`input_*_btn`/`input_*_axis`) — otherwise it's a well-formed but useless
/// file. This is a structural check only: it never launches RetroArch.
pub fn validate_autoconfig(text: &str) -> Result<ControllerProfile, String> {
    let profile = parse_autoconfig(text);
    if profile.get("input_driver").is_none() {
        return Err("missing input_driver".to_string());
    }
    if profile.get("input_device").is_none() {
        return Err("missing input_device".to_string());
    }
    let has_mapping = profile
        .entries
        .iter()
        .any(|(k, _)| k.starts_with("input_") && (k.ends_with("_btn") || k.ends_with("_axis")));
    if !has_mapping {
        return Err("no button/axis mapping (input_*_btn / input_*_axis) found".to_string());
    }
    Ok(profile)
}

/// Two commonly-used starting-point profiles (name, `.cfg` content) seeded
/// into the library the first time it's needed. These are typical/standard
/// mappings, not guaranteed correct for every physical unit of every pad —
/// the whole point of the library being plain editable `.cfg` files is that
/// the user corrects/adds to them without recompiling anything.
pub fn starter_profiles() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "xbox360-xinput.cfg",
            r#"input_driver = "xinput"
input_device = "Xbox 360 Controller"
input_b_btn = "0"
input_y_btn = "2"
input_select_btn = "6"
input_start_btn = "7"
input_up_axis = "-1"
input_down_axis = "+1"
input_left_axis = "-0"
input_right_axis = "+0"
input_a_btn = "1"
input_x_btn = "3"
input_l_btn = "4"
input_r_btn = "5"
input_l2_axis = "+2"
input_r2_axis = "+5"
input_l3_btn = "9"
input_r3_btn = "10"
"#,
        ),
        (
            "generic-directinput.cfg",
            r#"input_driver = "dinput"
input_device = "Generic USB Gamepad"
input_b_btn = "1"
input_y_btn = "3"
input_select_btn = "8"
input_start_btn = "9"
input_up_btn = "h0up"
input_down_btn = "h0down"
input_left_btn = "h0left"
input_right_btn = "h0right"
input_a_btn = "0"
input_x_btn = "2"
input_l_btn = "4"
input_r_btn = "5"
input_l2_btn = "6"
input_r2_btn = "7"
input_l3_btn = "10"
input_r3_btn = "11"
"#,
        ),
    ]
}

fn library_dir() -> PathBuf {
    retrotools_common::config::plugin_data_dir_path("controllers")
        .map(|dir| dir.join("profiles"))
        .unwrap_or_else(|_| PathBuf::from("profiles"))
}

fn seed_default_library(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (filename, content) in starter_profiles() {
        let path = dir.join(filename);
        if !path.exists() {
            std::fs::write(path, content)?;
        }
    }
    Ok(())
}

pub type ValidProfiles = Vec<(PathBuf, ControllerProfile)>;
pub type InvalidProfiles = Vec<(PathBuf, String)>;

/// Every `.cfg` file directly under `dir`, split into ones that parse as a
/// structurally valid autoconfig profile and ones that don't (with why).
pub fn list_library(dir: &Path) -> std::io::Result<(ValidProfiles, InvalidProfiles)> {
    let mut valid = Vec::new();
    let mut invalid = Vec::new();
    if !dir.is_dir() {
        return Ok((valid, invalid));
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("cfg"))
        .collect();
    paths.sort();
    for path in paths {
        let text = std::fs::read_to_string(&path)?;
        match validate_autoconfig(&text) {
            Ok(profile) => valid.push((path, profile)),
            Err(reason) => invalid.push((path, reason)),
        }
    }
    Ok((valid, invalid))
}

/// Exports every valid profile in the controller library to `ctx.output_dir`
/// (the `autoconfig/` folder RetroArch reads on the target distribution).
/// Exports the whole library in one run rather than one profile at a time:
/// `PluginContext` has no per-run "pick one" field (see the Batocera export
/// plugin's doc comment for the same tradeoff), and syncing an entire
/// curated library to a freshly-set-up device is the more useful operation
/// anyway.
pub struct ControllerExportPlugin;

impl Plugin for ControllerExportPlugin {
    fn id(&self) -> &'static str {
        "controllers-export"
    }

    fn name(&self) -> &'static str {
        "Controller Profiles Export"
    }

    fn description(&self) -> &'static str {
        "Export every valid RetroArch autoconfig profile in your controller library to a target autoconfig/ folder."
    }

    fn run(&self, ctx: &PluginContext) -> PluginResult<PluginOutcome> {
        let owned_default;
        let source_dir: &Path = match ctx.source_dir {
            Some(dir) => dir,
            None => {
                let dir = library_dir();
                seed_default_library(&dir).map_err(|e| e.to_string())?;
                owned_default = dir;
                &owned_default
            }
        };

        let (valid, invalid) = list_library(source_dir).map_err(|e| e.to_string())?;
        if valid.is_empty() && invalid.is_empty() {
            return Err(format!(
                "no .cfg profiles found in '{}'",
                source_dir.display()
            ));
        }

        if ctx.dry_run {
            let mut summary = format!(
                "[dry run] would export {} valid profile(s) to '{}'",
                valid.len(),
                ctx.output_dir.display()
            );
            if !invalid.is_empty() {
                summary.push_str(&format!("; {} profile(s) skipped (invalid)", invalid.len()));
            }
            return Ok(PluginOutcome {
                summary,
                files_written: Vec::new(),
            });
        }

        std::fs::create_dir_all(ctx.output_dir).map_err(|e| e.to_string())?;
        let mut files_written = Vec::new();
        for (path, _) in &valid {
            let file_name = path.file_name().ok_or("profile path has no file name")?;
            let dest = ctx.output_dir.join(file_name);
            std::fs::copy(path, &dest)
                .map_err(|e| format!("cannot copy '{}': {e}", path.display()))?;
            files_written.push(dest);
        }

        let mut summary = format!(
            "exported {} controller profile(s) to '{}'",
            files_written.len(),
            ctx.output_dir.display()
        );
        if !invalid.is_empty() {
            let names: Vec<String> = invalid
                .iter()
                .map(|(p, reason)| {
                    format!(
                        "{} ({reason})",
                        p.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    )
                })
                .collect();
            summary.push_str(&format!(
                " — skipped invalid profile(s): {}",
                names.join(", ")
            ));
        }

        Ok(PluginOutcome {
            summary,
            files_written,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retrotools_core::{DatHeader, DatType, GameSet};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rt26-plugin-controllers-test-{name}-{}",
            std::process::id()
        ));
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
    fn parse_then_serialize_round_trips() {
        let (_, text) = starter_profiles()[0];
        let profile = parse_autoconfig(text);
        assert_eq!(profile.device_name(), Some("Xbox 360 Controller"));
        let reserialized = serialize_autoconfig(&profile);
        let reparsed = parse_autoconfig(&reserialized);
        assert_eq!(profile, reparsed);
    }

    #[test]
    fn validate_rejects_a_profile_missing_required_fields() {
        assert!(validate_autoconfig("input_driver = \"xinput\"\n").is_err());
        assert!(validate_autoconfig("input_device = \"Pad\"\n").is_err());
        assert!(
            validate_autoconfig("input_driver = \"xinput\"\ninput_device = \"Pad\"\n").is_err()
        );
    }

    #[test]
    fn validate_accepts_every_starter_profile() {
        for (name, text) in starter_profiles() {
            assert!(
                validate_autoconfig(text).is_ok(),
                "starter profile '{name}' should be valid"
            );
        }
    }

    #[test]
    fn exports_valid_profiles_and_skips_invalid_ones() {
        let library = temp_dir("export-library");
        std::fs::write(library.join("good.cfg"), starter_profiles()[0].1).unwrap();
        std::fs::write(library.join("bad.cfg"), "not a valid profile\n").unwrap();
        let output = temp_dir("export-output");
        let gs = empty_gameset();

        let ctx = PluginContext {
            gameset: &gs,
            kept_game_names: &[],
            source_dir: Some(&library),
            output_dir: &output,
            match_report: None,
            dry_run: false,
        };
        let outcome = ControllerExportPlugin.run(&ctx).unwrap();
        assert_eq!(outcome.files_written.len(), 1);
        assert!(outcome.summary.contains("skipped invalid"));
        assert!(output.join("good.cfg").is_file());
        assert!(!output.join("bad.cfg").exists());

        std::fs::remove_dir_all(&library).ok();
        std::fs::remove_dir_all(&output).ok();
    }

    #[test]
    fn dry_run_reports_the_plan_without_writing_anything() {
        let library = temp_dir("dry-library");
        std::fs::write(library.join("good.cfg"), starter_profiles()[0].1).unwrap();
        let output = temp_dir("dry-output");
        let gs = empty_gameset();

        let ctx = PluginContext {
            gameset: &gs,
            kept_game_names: &[],
            source_dir: Some(&library),
            output_dir: &output,
            match_report: None,
            dry_run: true,
        };
        let outcome = ControllerExportPlugin.run(&ctx).unwrap();
        assert!(outcome.summary.starts_with("[dry run]"));
        assert!(!output.join("good.cfg").exists());

        std::fs::remove_dir_all(&library).ok();
        std::fs::remove_dir_all(&output).ok();
    }

    #[test]
    fn no_source_dir_seeds_and_uses_the_default_library() {
        let output = temp_dir("seeded-output");
        let gs = empty_gameset();

        let ctx = PluginContext {
            gameset: &gs,
            kept_game_names: &[],
            source_dir: None,
            output_dir: &output,
            match_report: None,
            dry_run: false,
        };
        let outcome = ControllerExportPlugin.run(&ctx).unwrap();
        assert_eq!(outcome.files_written.len(), starter_profiles().len());

        std::fs::remove_dir_all(&output).ok();
    }
}

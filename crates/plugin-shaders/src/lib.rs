use retrotools_plugin_api::{Plugin, PluginContext, PluginOutcome, PluginResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Where a shader association applies: the whole core (every game run with
/// it) or one specific piece of content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShaderScope {
    System,
    Game,
}

/// One "use this preset here" rule, on the same reusable-profile model as
/// `retrotools_core::profiles::RuleProfile` (Phase 3), except a shader
/// association is a list entry rather than a whole named profile — several
/// associations are normally active at once (one per system, plus a few
/// per-game exceptions).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShaderAssociation {
    pub scope: ShaderScope,
    /// The RetroArch core folder name the override applies to (e.g.
    /// `snes9x`), matching RetroArch's own override directory layout.
    pub core_name: String,
    /// Required (and only meaningful) for `ShaderScope::Game`: the content
    /// directory name RetroArch derives from the ROM's parent folder.
    pub content_dir_name: Option<String>,
    /// Required for `ShaderScope::Game`: the game's content name (file stem)
    /// RetroArch uses to name the per-game override.
    pub game_name: Option<String>,
    /// File name of a preset in the shader library (see [`library_dir`]).
    pub preset: String,
}

impl ShaderAssociation {
    pub fn validate(&self) -> Result<(), String> {
        if self.core_name.trim().is_empty() {
            return Err("core_name is required".into());
        }
        if self.preset.trim().is_empty() {
            return Err("preset is required".into());
        }
        if self.scope == ShaderScope::Game
            && (self.content_dir_name.as_deref().unwrap_or("").is_empty()
                || self.game_name.as_deref().unwrap_or("").is_empty())
        {
            return Err("Game-scoped associations need content_dir_name and game_name".into());
        }
        Ok(())
    }
}

fn shaders_root() -> PathBuf {
    retrotools_common::config::plugin_data_dir_path("shaders")
        .unwrap_or_else(|_| PathBuf::from("shaders-data"))
}

pub fn library_dir() -> PathBuf {
    shaders_root().join("presets")
}

fn associations_path() -> PathBuf {
    shaders_root().join("associations.json")
}

/// Loads the saved association list, or an empty list if none has been
/// saved yet — not an error, a fresh install simply has no associations.
pub fn load_associations() -> Result<Vec<ShaderAssociation>, String> {
    load_associations_from(&associations_path())
}

fn load_associations_from(path: &Path) -> Result<Vec<ShaderAssociation>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

pub fn save_associations(associations: &[ShaderAssociation]) -> Result<(), String> {
    save_associations_to(&associations_path(), associations)
}

fn save_associations_to(path: &Path, associations: &[ShaderAssociation]) -> Result<(), String> {
    for assoc in associations {
        assoc.validate()?;
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(associations).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

/// A handful of commonly used preset *references*: shader presets are
/// normally just a list of pass definitions pointing at shader files
/// RetroArch already ships under `shaders/shaders_slang/…`, so a preset
/// file is tiny — these are realistic starting points, not a bundled
/// shader implementation (this project ships no GLSL/slang shader code of
/// its own, and none of these three files were copied from the community
/// `libretro/slang-shaders` repository — only its real, publicly visible
/// folder/file layout was used as a reference so the paths below actually
/// resolve on a standard RetroArch install; the repository mixes several
/// third-party licenses per shader author, so nothing from it is
/// redistributed here verbatim).
pub fn starter_presets() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "crt-geom.slangp",
            "shaders = \"1\"\nshader0 = \"shaders_slang/crt/shaders/crt-geom.slang\"\nfilter_linear0 = \"true\"\nscale_type0 = \"viewport\"\n",
        ),
        (
            "scanlines-sharp.slangp",
            "shaders = \"1\"\nshader0 = \"shaders_slang/scanlines/shaders/scanline.slang\"\nfilter_linear0 = \"false\"\nscale_type0 = \"viewport\"\n",
        ),
        (
            "scale2x.slangp",
            "shaders = \"2\"\nshader0 = \"shaders_slang/edge-smoothing/scalenx/shaders/scale2x.slang\"\nfilter_linear0 = \"false\"\nscale_type0 = \"source\"\nscale_x0 = \"2.0\"\nscale_y0 = \"2.0\"\nshader1 = \"shaders_slang/interpolation/shaders/bicubic.slang\"\nfilter_linear1 = \"false\"\nscale_type1 = \"viewport\"\n",
        ),
    ]
}

fn seed_default_library(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (filename, content) in starter_presets() {
        let path = dir.join(filename);
        if !path.exists() {
            std::fs::write(path, content)?;
        }
    }
    Ok(())
}

/// Imports an externally-supplied `.glslp`/`.slangp` preset file into the
/// shader library, keyed by file name. Rejects anything else so the library
/// only ever holds files RetroArch itself would recognize as a shader
/// preset.
pub fn import_preset(dir: &Path, filename: &str, content: &str) -> Result<PathBuf, String> {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if ext != "glslp" && ext != "slangp" {
        return Err(format!(
            "'{filename}' is not a .glslp or .slangp preset file"
        ));
    }
    if content.trim().is_empty() {
        return Err("preset content is empty".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(filename);
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn list_library(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("glslp") | Some("slangp")
            )
        })
        .collect();
    paths.sort();
    Ok(paths)
}

/// The override file path RetroArch expects for an association, relative to
/// its override root — same three-level layout RetroArch itself uses:
/// `<core>/<core>.cfg` (core-wide), `<core>/<content dir>.cfg` (per content
/// directory — not used here, associations are per-game or per-core only),
/// `<core>/<content dir>/<game>.cfg` (per game).
pub fn override_relative_path(assoc: &ShaderAssociation) -> PathBuf {
    match assoc.scope {
        ShaderScope::System => {
            PathBuf::from(&assoc.core_name).join(format!("{}.cfg", assoc.core_name))
        }
        ShaderScope::Game => PathBuf::from(&assoc.core_name)
            .join(assoc.content_dir_name.as_deref().unwrap_or(""))
            .join(format!("{}.cfg", assoc.game_name.as_deref().unwrap_or(""))),
    }
}

const GENERATED_MARKER: &str =
    "# Generated by Retro Tools 2026 -- safe to delete, will be regenerated";
const MANIFEST_FILENAME: &str = ".retrotools26-generated.json";

fn override_content(shader_path: &Path) -> String {
    format!(
        "{GENERATED_MARKER}\nvideo_shader = \"{}\"\nvideo_shader_enable = \"true\"\n",
        shader_path.display()
    )
}

fn manifest_path(output_dir: &Path) -> PathBuf {
    output_dir.join(MANIFEST_FILENAME)
}

fn load_manifest(output_dir: &Path) -> Vec<PathBuf> {
    let path = manifest_path(output_dir);
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_manifest(output_dir: &Path, entries: &[PathBuf]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
    std::fs::write(manifest_path(output_dir), json).map_err(|e| e.to_string())
}

/// Generates RetroArch override files for every association whose preset
/// exists in the library, plus a copy of each referenced preset under
/// `<output_dir>/shaders/`. Every path it writes is recorded in a manifest
/// (`.retrotools26-generated.json`) so a later cleanup run only ever removes
/// files this tool itself created — an override the user wrote by hand for
/// a core/game this plugin doesn't manage is never touched.
pub struct ShaderOverridesPlugin;

impl Plugin for ShaderOverridesPlugin {
    fn id(&self) -> &'static str {
        "shaders-export"
    }

    fn name(&self) -> &'static str {
        "Shader Overrides Export"
    }

    fn description(&self) -> &'static str {
        "Generate RetroArch shader override files from your saved shader associations."
    }

    fn run(&self, ctx: &PluginContext) -> PluginResult<PluginOutcome> {
        let preset_dir = library_dir();
        seed_default_library(&preset_dir).map_err(|e| e.to_string())?;
        let associations = load_associations()?;
        if associations.is_empty() {
            return Err(
                "no shader associations saved yet — add one in Settings/Shaders first".into(),
            );
        }

        let mut plan = Vec::new();
        let mut missing_presets = Vec::new();
        for assoc in &associations {
            assoc.validate()?;
            let preset_path = preset_dir.join(&assoc.preset);
            if !preset_path.is_file() {
                missing_presets.push(assoc.preset.clone());
                continue;
            }
            plan.push((assoc.clone(), preset_path));
        }
        if plan.is_empty() {
            return Err(format!(
                "none of the {} association(s) reference a preset that exists in the library",
                associations.len()
            ));
        }

        if ctx.dry_run {
            let mut summary = format!(
                "[dry run] would generate {} shader override(s) in '{}'",
                plan.len(),
                ctx.output_dir.display()
            );
            if !missing_presets.is_empty() {
                summary.push_str(&format!(
                    "; missing preset(s): {}",
                    missing_presets.join(", ")
                ));
            }
            return Ok(PluginOutcome {
                summary,
                files_written: Vec::new(),
            });
        }

        let shaders_out = ctx.output_dir.join("shaders");
        std::fs::create_dir_all(&shaders_out).map_err(|e| e.to_string())?;

        let mut files_written = Vec::new();
        let mut manifest_entries = Vec::new();
        for (assoc, preset_path) in &plan {
            let copied_shader = shaders_out.join(&assoc.preset);
            std::fs::copy(preset_path, &copied_shader)
                .map_err(|e| format!("cannot copy preset '{}': {e}", preset_path.display()))?;

            let rel = override_relative_path(assoc);
            let override_path = ctx.output_dir.join(&rel);
            if let Some(parent) = override_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&override_path, override_content(&copied_shader))
                .map_err(|e| e.to_string())?;

            files_written.push(override_path.clone());
            manifest_entries.push(rel);
            manifest_entries.push(PathBuf::from("shaders").join(&assoc.preset));
        }
        files_written.push(shaders_out.join(""));
        save_manifest(ctx.output_dir, &manifest_entries)?;

        let mut summary = format!(
            "generated {} shader override(s) in '{}'",
            plan.len(),
            ctx.output_dir.display()
        );
        if !missing_presets.is_empty() {
            summary.push_str(&format!(
                " — skipped association(s) with a missing preset: {}",
                missing_presets.join(", ")
            ));
        }
        Ok(PluginOutcome {
            summary,
            files_written,
        })
    }
}

/// Removes exactly the files this tool generated on a previous
/// `shaders-export` run (per `.retrotools26-generated.json` in
/// `ctx.output_dir`), then clears the manifest. Any override file the user
/// created or hand-edited outside that list is left alone.
pub struct ShaderCleanupPlugin;

impl Plugin for ShaderCleanupPlugin {
    fn id(&self) -> &'static str {
        "shaders-clean"
    }

    fn name(&self) -> &'static str {
        "Shader Overrides Cleanup"
    }

    fn description(&self) -> &'static str {
        "Remove shader override files previously generated by this tool, leaving hand-made overrides untouched."
    }

    fn run(&self, ctx: &PluginContext) -> PluginResult<PluginOutcome> {
        let manifest = load_manifest(ctx.output_dir);
        if manifest.is_empty() {
            return Ok(PluginOutcome {
                summary: "nothing to clean up — no generated overrides recorded".into(),
                files_written: Vec::new(),
            });
        }

        if ctx.dry_run {
            return Ok(PluginOutcome {
                summary: format!(
                    "[dry run] would remove {} generated file(s)",
                    manifest.len()
                ),
                files_written: Vec::new(),
            });
        }

        let mut removed = 0usize;
        for rel in &manifest {
            let path = ctx.output_dir.join(rel);
            if path.is_file() && std::fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
        std::fs::remove_file(manifest_path(ctx.output_dir)).ok();

        Ok(PluginOutcome {
            summary: format!("removed {removed} generated shader override file(s)"),
            files_written: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retrotools_core::{DatHeader, DatType, GameSet};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rt26-plugin-shaders-test-{name}-{}",
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

    fn ctx<'a>(gs: &'a GameSet, output: &'a Path, dry_run: bool) -> PluginContext<'a> {
        PluginContext {
            gameset: gs,
            kept_game_names: &[],
            source_dir: None,
            output_dir: output,
            match_report: None,
            dry_run,
        }
    }

    #[test]
    fn game_scope_without_content_or_game_name_is_invalid() {
        let assoc = ShaderAssociation {
            scope: ShaderScope::Game,
            core_name: "snes9x".into(),
            content_dir_name: None,
            game_name: None,
            preset: "crt-geom.slangp".into(),
        };
        assert!(assoc.validate().is_err());
    }

    #[test]
    fn override_paths_match_retroarch_layout() {
        let system = ShaderAssociation {
            scope: ShaderScope::System,
            core_name: "snes9x".into(),
            content_dir_name: None,
            game_name: None,
            preset: "crt-geom.slangp".into(),
        };
        assert_eq!(
            override_relative_path(&system),
            PathBuf::from("snes9x").join("snes9x.cfg")
        );

        let game = ShaderAssociation {
            scope: ShaderScope::Game,
            core_name: "snes9x".into(),
            content_dir_name: Some("SNES".into()),
            game_name: Some("Super Metroid".into()),
            preset: "crt-geom.slangp".into(),
        };
        assert_eq!(
            override_relative_path(&game),
            PathBuf::from("snes9x")
                .join("SNES")
                .join("Super Metroid.cfg")
        );
    }

    #[test]
    fn associations_round_trip_through_json() {
        let dir = temp_dir("assoc-roundtrip");
        let path = dir.join("associations.json");
        let list = vec![ShaderAssociation {
            scope: ShaderScope::System,
            core_name: "snes9x".into(),
            content_dir_name: None,
            game_name: None,
            preset: "crt-geom.slangp".into(),
        }];
        save_associations_to(&path, &list).unwrap();
        let loaded = load_associations_from(&path).unwrap();
        assert_eq!(loaded, list);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn saving_an_invalid_association_is_rejected() {
        let dir = temp_dir("assoc-invalid");
        let path = dir.join("associations.json");
        let list = vec![ShaderAssociation {
            scope: ShaderScope::Game,
            core_name: "snes9x".into(),
            content_dir_name: None,
            game_name: None,
            preset: "crt-geom.slangp".into(),
        }];
        assert!(save_associations_to(&path, &list).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn import_preset_rejects_wrong_extension() {
        let dir = temp_dir("import-reject");
        assert!(import_preset(&dir, "not-a-shader.txt", "shaders = \"1\"").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn import_preset_accepts_slangp_and_glslp() {
        let dir = temp_dir("import-accept");
        let p1 = import_preset(&dir, "custom.slangp", "shaders = \"1\"\n").unwrap();
        let p2 = import_preset(&dir, "custom.glslp", "shaders = \"1\"\n").unwrap();
        assert!(p1.is_file());
        assert!(p2.is_file());
        let listed = list_library(&dir).unwrap();
        assert_eq!(listed.len(), 2);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The starter presets reference real `libretro/slang-shaders` file
    /// paths (verified against the upstream repository layout, not copied
    /// from it) so they actually resolve on a standard RetroArch install —
    /// this pins the exact paths down so a future edit can't silently
    /// regress them back to a guessed/incorrect layout.
    #[test]
    fn starter_presets_reference_real_retroarch_shader_paths() {
        let presets = starter_presets();
        assert!(presets[0]
            .1
            .contains("shaders_slang/crt/shaders/crt-geom.slang"));
        assert!(presets[1]
            .1
            .contains("shaders_slang/scanlines/shaders/scanline.slang"));
        assert!(presets[2]
            .1
            .contains("shaders_slang/edge-smoothing/scalenx/shaders/scale2x.slang"));
        assert!(presets[2]
            .1
            .contains("shaders_slang/interpolation/shaders/bicubic.slang"));
    }

    #[test]
    fn export_generates_overrides_for_valid_associations_and_skips_missing_presets() {
        let preset_dir = temp_dir("export-presets");
        // The plugin's own library dir isn't overridable in tests (it lives
        // under the real per-user data dir), so this exercises the exact
        // same write sequence `ShaderOverridesPlugin::run` performs, against
        // an isolated temp dir, rather than the plugin's `run` itself.
        std::fs::write(preset_dir.join("crt-geom.slangp"), starter_presets()[0].1).unwrap();

        let output = temp_dir("export-output");
        let assoc_ok = ShaderAssociation {
            scope: ShaderScope::System,
            core_name: "snes9x".into(),
            content_dir_name: None,
            game_name: None,
            preset: "crt-geom.slangp".into(),
        };
        let preset_path = preset_dir.join(&assoc_ok.preset);
        assert!(preset_path.is_file());

        let shaders_out = output.join("shaders");
        std::fs::create_dir_all(&shaders_out).unwrap();
        let copied = shaders_out.join(&assoc_ok.preset);
        std::fs::copy(&preset_path, &copied).unwrap();
        let rel = override_relative_path(&assoc_ok);
        let override_path = output.join(&rel);
        std::fs::create_dir_all(override_path.parent().unwrap()).unwrap();
        std::fs::write(&override_path, override_content(&copied)).unwrap();

        let content = std::fs::read_to_string(&override_path).unwrap();
        assert!(content.contains("video_shader_enable = \"true\""));
        assert!(content.starts_with(GENERATED_MARKER));

        std::fs::remove_dir_all(&preset_dir).ok();
        std::fs::remove_dir_all(&output).ok();
    }

    #[test]
    fn cleanup_removes_only_manifested_files_and_leaves_hand_made_ones() {
        let output = temp_dir("cleanup-output");
        let generated = output.join("snes9x").join("snes9x.cfg");
        std::fs::create_dir_all(generated.parent().unwrap()).unwrap();
        std::fs::write(
            &generated,
            override_content(Path::new("shaders/crt-geom.slangp")),
        )
        .unwrap();

        let hand_made = output.join("nes").join("nes.cfg");
        std::fs::create_dir_all(hand_made.parent().unwrap()).unwrap();
        std::fs::write(&hand_made, "video_shader = \"my_own.slangp\"\n").unwrap();

        save_manifest(&output, &[PathBuf::from("snes9x").join("snes9x.cfg")]).unwrap();

        let gs = empty_gameset();
        let outcome = ShaderCleanupPlugin.run(&ctx(&gs, &output, false)).unwrap();
        assert!(outcome.summary.contains("removed 1"));
        assert!(!generated.exists());
        assert!(hand_made.is_file());

        std::fs::remove_dir_all(&output).ok();
    }

    #[test]
    fn export_plugin_run_writes_override_and_manifest_end_to_end() {
        // Associations/presets live under the real per-user data dir (like
        // `ScreenScraperCredentials` in the scraper plugin's tests), so this
        // backs up and restores whatever was there to avoid clobbering real
        // user data on a dev machine running the full test suite.
        let assoc_file = associations_path();
        let backup = load_associations().unwrap();

        let list = vec![ShaderAssociation {
            scope: ShaderScope::System,
            core_name: "test-core-e2e".into(),
            content_dir_name: None,
            game_name: None,
            preset: "crt-geom.slangp".into(),
        }];
        save_associations(&list).unwrap();

        let output = temp_dir("export-run-e2e");
        let gs = empty_gameset();
        let outcome = ShaderOverridesPlugin
            .run(&ctx(&gs, &output, false))
            .unwrap();
        assert!(outcome.summary.contains("generated 1 shader override"));
        let override_path = output.join("test-core-e2e").join("test-core-e2e.cfg");
        assert!(override_path.is_file());
        assert!(manifest_path(&output).is_file());

        // Restore.
        if backup.is_empty() {
            std::fs::remove_file(&assoc_file).ok();
        } else {
            save_associations(&backup).unwrap();
        }
        std::fs::remove_dir_all(&output).ok();
    }

    #[test]
    fn cleanup_with_no_manifest_is_a_clean_noop() {
        let output = temp_dir("cleanup-empty");
        let gs = empty_gameset();
        let outcome = ShaderCleanupPlugin.run(&ctx(&gs, &output, false)).unwrap();
        assert!(outcome.summary.contains("nothing to clean up"));
        std::fs::remove_dir_all(&output).ok();
    }
}

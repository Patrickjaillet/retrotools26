use retrotools_plugin_api::{Plugin, PluginContext, PluginOutcome, PluginResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Distro {
    Batocera,
    Recalbox,
    Lakka,
}

impl Distro {
    fn plugin_id(self) -> &'static str {
        match self {
            Distro::Batocera => "export-batocera",
            Distro::Recalbox => "export-recalbox",
            Distro::Lakka => "export-lakka",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Distro::Batocera => "Batocera",
            Distro::Recalbox => "Recalbox",
            Distro::Lakka => "Lakka",
        }
    }

    /// Both Batocera and Recalbox are EmulationStation forks that read an
    /// `es_systems.cfg`; Lakka runs RetroArch directly with no
    /// EmulationStation front-end, so there is no equivalent file to
    /// generate for it — only the ROM folder tree applies.
    fn uses_es_systems_cfg(self) -> bool {
        matches!(self, Distro::Batocera | Distro::Recalbox)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMapping {
    pub platform: String,
    #[serde(default)]
    pub batocera: Vec<String>,
    #[serde(default)]
    pub recalbox: Vec<String>,
    #[serde(default)]
    pub lakka: Vec<String>,
}

impl SystemMapping {
    fn folders_for(&self, distro: Distro) -> &[String] {
        match distro {
            Distro::Batocera => &self.batocera,
            Distro::Recalbox => &self.recalbox,
            Distro::Lakka => &self.lakka,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemTable {
    pub systems: Vec<SystemMapping>,
}

impl SystemTable {
    pub fn find(&self, platform: &str) -> Option<&SystemMapping> {
        self.systems.iter().find(|s| s.platform.eq_ignore_ascii_case(platform))
    }

    /// Folder names to export `platform` into for `distro`. Falls back to a
    /// single slugified version of the platform name when nothing is
    /// mapped, so an unknown/custom platform still exports somewhere
    /// sensible instead of silently doing nothing.
    pub fn folder_names_for(&self, platform: &str, distro: Distro) -> (Vec<String>, bool) {
        if let Some(mapping) = self.find(platform) {
            let folders = mapping.folders_for(distro);
            if !folders.is_empty() {
                return (folders.to_vec(), false);
            }
        }
        (vec![slugify(platform)], true)
    }
}

fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut last_was_dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// A reasonable built-in starting table covering common No-Intro/Redump
/// platform names. Not exhaustive by design — it's an editable JSON file
/// the user extends for anything missing, not a hardcoded final list.
pub fn default_system_table() -> SystemTable {
    fn m(platform: &str, batocera: &[&str], recalbox: &[&str], lakka: &[&str]) -> SystemMapping {
        SystemMapping {
            platform: platform.to_string(),
            batocera: batocera.iter().map(|s| s.to_string()).collect(),
            recalbox: recalbox.iter().map(|s| s.to_string()).collect(),
            lakka: lakka.iter().map(|s| s.to_string()).collect(),
        }
    }
    SystemTable {
        systems: vec![
            m("Nintendo - Nintendo Entertainment System", &["nes"], &["nes"], &["nes"]),
            m("Nintendo - Super Nintendo Entertainment System", &["snes"], &["snes"], &["snes"]),
            m("Nintendo - Game Boy", &["gb"], &["gb"], &["gb"]),
            m("Nintendo - Game Boy Color", &["gbc"], &["gbc"], &["gbc"]),
            m("Nintendo - Game Boy Advance", &["gba"], &["gba"], &["gba"]),
            m("Nintendo - Nintendo 64", &["n64"], &["n64"], &["n64"]),
            m("Sega - Mega Drive - Genesis", &["megadrive"], &["megadrive"], &["genesis"]),
            m("Sega - Master System - Mark III", &["mastersystem"], &["mastersystem"], &["sms"]),
            m("Sega - Game Gear", &["gamegear"], &["gamegear"], &["gamegear"]),
            m("Sega - Saturn", &["saturn"], &["saturn"], &["saturn"]),
            m("Sony - PlayStation", &["psx"], &["psx"], &["psx"]),
            m("Sony - PlayStation Portable", &["psp"], &["psp"], &["psp"]),
            m("Atari - 2600", &["atari2600"], &["atari2600"], &["atari2600"]),
            m("NEC - PC Engine - TurboGrafx 16", &["pcengine"], &["pcengine"], &["pcengine"]),
            m("SNK - Neo Geo", &["neogeo"], &["neogeo"], &["neogeo"]),
            m("MAME", &["mame"], &["mame"], &["mame"]),
        ],
    }
}

pub fn load_or_default_table(path: &Path) -> SystemTable {
    if let Ok(raw) = std::fs::read_to_string(path) {
        if let Ok(table) = serde_json::from_str::<SystemTable>(&raw) {
            return table;
        }
    }
    default_system_table()
}

pub fn save_table(path: &Path, table: &SystemTable) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(table).expect("SystemTable always serializes");
    std::fs::write(path, json)
}

fn table_path() -> PathBuf {
    retrotools_common::config::plugin_data_dir_path("batocera-export")
        .map(|dir| dir.join("systems.json"))
        .unwrap_or_else(|_| PathBuf::from("systems.json"))
}

fn collect_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Replaces (or appends) the `<system><name>{key}</name>...</system>` block
/// for `key` in an existing `es_systems.cfg`, leaving every other system's
/// entry untouched — re-exporting the same platform is idempotent, and
/// exporting a second platform never erases the first one's entry.
fn merge_es_systems_cfg(existing: &str, key: &str, fragment: &str) -> String {
    let open_tag = format!("<name>{key}</name>");
    let mut result = String::new();
    let rest;
    if let Some(name_pos) = existing.find(&open_tag) {
        let block_start = existing[..name_pos].rfind("<system>").unwrap_or(0);
        let block_end = existing[name_pos..]
            .find("</system>")
            .map(|i| name_pos + i + "</system>".len())
            .unwrap_or(existing.len());
        result.push_str(&existing[..block_start]);
        result.push_str(fragment);
        rest = &existing[block_end..];
    } else if let Some(close_pos) = existing.rfind("</systemList>") {
        result.push_str(&existing[..close_pos]);
        result.push_str(fragment);
        rest = &existing[close_pos..];
    } else {
        result.push_str("<?xml version=\"1.0\"?>\n<systemList>\n");
        result.push_str(fragment);
        result.push_str("</systemList>\n");
        return result;
    }
    result.push_str(rest);
    result
}

fn system_fragment(key: &str, folder_name: &str) -> String {
    format!(
        "  <system>\n    <name>{key}</name>\n    <fullname>{key}</fullname>\n    <path>./{folder_name}</path>\n    <extension>.zip .7z</extension>\n  </system>\n"
    )
}

pub struct BatoceraExportPlugin {
    pub distro: Distro,
}

impl Plugin for BatoceraExportPlugin {
    fn id(&self) -> &'static str {
        self.distro.plugin_id()
    }

    fn name(&self) -> &'static str {
        match self.distro {
            Distro::Batocera => "Batocera Export",
            Distro::Recalbox => "Recalbox Export",
            Distro::Lakka => "Lakka Export",
        }
    }

    fn description(&self) -> &'static str {
        match self.distro {
            Distro::Batocera => "Copy an already-built 1G1R set into a Batocera roms/<system>/ tree.",
            Distro::Recalbox => {
                "Copy an already-built 1G1R set into a Recalbox roms/<system>/ tree and update es_systems.cfg."
            }
            Distro::Lakka => "Copy an already-built 1G1R set into a Lakka roms/<system>/ tree.",
        }
    }

    fn run(&self, ctx: &PluginContext) -> PluginResult<PluginOutcome> {
        let source_dir = ctx
            .source_dir
            .ok_or_else(|| "this plugin needs a source folder: the already-built 1G1R set to export".to_string())?;
        if !source_dir.is_dir() {
            return Err(format!("source folder '{}' does not exist", source_dir.display()));
        }

        let table = load_or_default_table(&table_path());
        let (folder_names, used_fallback) = table.folder_names_for(&ctx.gameset.platform, self.distro);

        let files = collect_files(source_dir).map_err(|e| e.to_string())?;
        if files.is_empty() {
            return Err(format!("no files found in source folder '{}'", source_dir.display()));
        }

        let mut planned = Vec::new();
        for folder_name in &folder_names {
            let dest_dir = ctx.output_dir.join("roms").join(folder_name);
            for file in &files {
                let rel = file.strip_prefix(source_dir).unwrap_or(file);
                planned.push((file.clone(), dest_dir.join(rel)));
            }
        }

        if ctx.dry_run {
            let mut summary = format!(
                "[dry run] would copy {} file(s) into {} system folder(s) ({}) for {}",
                files.len(),
                folder_names.len(),
                folder_names.join(", "),
                self.distro.label()
            );
            if used_fallback {
                summary.push_str(&format!(
                    "; '{}' isn't in the system table, falling back to folder name '{}' — edit {} to add a real mapping",
                    ctx.gameset.platform,
                    folder_names[0],
                    table_path().display()
                ));
            }
            if self.distro.uses_es_systems_cfg() {
                summary.push_str("; would also add/update an es_systems.cfg entry");
            }
            return Ok(PluginOutcome { summary, files_written: Vec::new() });
        }

        let mut files_written = Vec::new();
        for (source, dest) in &planned {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::copy(source, dest).map_err(|e| format!("cannot copy '{}': {e}", source.display()))?;
            files_written.push(dest.clone());
        }

        if self.distro.uses_es_systems_cfg() {
            let cfg_path = ctx.output_dir.join("es_systems.cfg");
            let existing = std::fs::read_to_string(&cfg_path).unwrap_or_default();
            let key = folder_names[0].clone();
            let fragment = system_fragment(&key, &folder_names[0]);
            let merged = merge_es_systems_cfg(&existing, &key, &fragment);
            std::fs::write(&cfg_path, merged).map_err(|e| e.to_string())?;
            files_written.push(cfg_path);
        }

        let mut summary = format!(
            "exported {} file(s) into {} for {} ({})",
            files_written.len(),
            folder_names.join(", "),
            self.distro.label(),
            ctx.gameset.platform
        );
        if used_fallback {
            summary.push_str(&format!(
                " — no mapping found, used fallback folder name; edit {} to add one",
                table_path().display()
            ));
        }

        Ok(PluginOutcome { summary, files_written })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retrotools_core::{DatHeader, DatType, GameSet};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rt26-batocera-export-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn gameset(platform: &str) -> GameSet {
        GameSet {
            platform: platform.to_string(),
            dat_name: platform.to_string(),
            dat_version: "1".into(),
            dat_type: DatType::NoIntro,
            header: DatHeader::default(),
            games: Vec::new(),
        }
    }

    #[test]
    fn exports_known_platform_into_the_mapped_folder() {
        let source = temp_dir("known-source");
        std::fs::write(source.join("Game A.zip"), b"1234").unwrap();
        let output = temp_dir("known-output");

        let gs = gameset("Nintendo - Super Nintendo Entertainment System");
        let ctx = PluginContext {
            gameset: &gs,
            kept_game_names: &[],
            source_dir: Some(&source),
            output_dir: &output,
            match_report: None,
            dry_run: false,
        };
        let plugin = BatoceraExportPlugin { distro: Distro::Recalbox };
        let outcome = plugin.run(&ctx).unwrap();
        assert!(outcome.summary.contains("snes"));

        let dest_file = output.join("roms").join("snes").join("Game A.zip");
        assert!(dest_file.is_file());
        assert_eq!(std::fs::read(&dest_file).unwrap(), b"1234");

        let cfg = std::fs::read_to_string(output.join("es_systems.cfg")).unwrap();
        assert!(cfg.contains("<name>snes</name>"));

        std::fs::remove_dir_all(&source).ok();
        std::fs::remove_dir_all(&output).ok();
    }

    #[test]
    fn unknown_platform_falls_back_to_a_slugified_folder_name() {
        let source = temp_dir("unknown-source");
        std::fs::write(source.join("Weird Game.zip"), b"x").unwrap();
        let output = temp_dir("unknown-output");

        let gs = gameset("Some Totally Unknown Platform!!");
        let ctx = PluginContext {
            gameset: &gs,
            kept_game_names: &[],
            source_dir: Some(&source),
            output_dir: &output,
            match_report: None,
            dry_run: false,
        };
        let plugin = BatoceraExportPlugin { distro: Distro::Batocera };
        let outcome = plugin.run(&ctx).unwrap();
        assert!(outcome.summary.contains("fallback"));
        assert!(output.join("roms").join("some-totally-unknown-platform").join("Weird Game.zip").is_file());

        std::fs::remove_dir_all(&source).ok();
        std::fs::remove_dir_all(&output).ok();
    }

    #[test]
    fn dry_run_reports_the_plan_without_writing_anything() {
        let source = temp_dir("dry-source");
        std::fs::write(source.join("Game A.zip"), b"1234").unwrap();
        let output = temp_dir("dry-output");

        let gs = gameset("Nintendo - Game Boy");
        let ctx = PluginContext {
            gameset: &gs,
            kept_game_names: &[],
            source_dir: Some(&source),
            output_dir: &output,
            match_report: None,
            dry_run: true,
        };
        let plugin = BatoceraExportPlugin { distro: Distro::Recalbox };
        let outcome = plugin.run(&ctx).unwrap();
        assert!(outcome.summary.starts_with("[dry run]"));
        assert!(outcome.files_written.is_empty());
        assert!(!output.join("roms").exists(), "dry run must not create the roms/ tree");

        std::fs::remove_dir_all(&source).ok();
        std::fs::remove_dir_all(&output).ok();
    }

    #[test]
    fn lakka_never_writes_an_es_systems_cfg() {
        let source = temp_dir("lakka-source");
        std::fs::write(source.join("Game A.zip"), b"1234").unwrap();
        let output = temp_dir("lakka-output");

        let gs = gameset("Nintendo - Game Boy");
        let ctx = PluginContext {
            gameset: &gs,
            kept_game_names: &[],
            source_dir: Some(&source),
            output_dir: &output,
            match_report: None,
            dry_run: false,
        };
        let plugin = BatoceraExportPlugin { distro: Distro::Lakka };
        plugin.run(&ctx).unwrap();
        assert!(!output.join("es_systems.cfg").exists());

        std::fs::remove_dir_all(&source).ok();
        std::fs::remove_dir_all(&output).ok();
    }

    #[test]
    fn re_exporting_the_same_platform_is_idempotent_and_a_second_platform_does_not_erase_the_first() {
        let source_a = temp_dir("merge-source-a");
        std::fs::write(source_a.join("A.zip"), b"a").unwrap();
        let source_b = temp_dir("merge-source-b");
        std::fs::write(source_b.join("B.zip"), b"b").unwrap();
        let output = temp_dir("merge-output");

        let gs_snes = gameset("Nintendo - Super Nintendo Entertainment System");
        let gs_nes = gameset("Nintendo - Nintendo Entertainment System");
        let plugin = BatoceraExportPlugin { distro: Distro::Recalbox };

        let ctx_snes = PluginContext {
            gameset: &gs_snes,
            kept_game_names: &[],
            source_dir: Some(&source_a),
            output_dir: &output,
            match_report: None,
            dry_run: false,
        };
        plugin.run(&ctx_snes).unwrap();
        plugin.run(&ctx_snes).unwrap(); // idempotent re-export

        let ctx_nes = PluginContext {
            gameset: &gs_nes,
            kept_game_names: &[],
            source_dir: Some(&source_b),
            output_dir: &output,
            match_report: None,
            dry_run: false,
        };
        plugin.run(&ctx_nes).unwrap();

        let cfg = std::fs::read_to_string(output.join("es_systems.cfg")).unwrap();
        assert_eq!(cfg.matches("<name>snes</name>").count(), 1, "no duplicate snes entry");
        assert_eq!(cfg.matches("<name>nes</name>").count(), 1, "nes entry present");
        assert!(output.join("roms").join("snes").join("A.zip").is_file());
        assert!(output.join("roms").join("nes").join("B.zip").is_file());

        std::fs::remove_dir_all(&source_a).ok();
        std::fs::remove_dir_all(&source_b).ok();
        std::fs::remove_dir_all(&output).ok();
    }

    #[test]
    fn slugify_handles_punctuation_and_repeated_separators() {
        assert_eq!(slugify("Sega - Mega Drive - Genesis"), "sega-mega-drive-genesis");
        assert_eq!(slugify("Atari 2600!!"), "atari-2600");
    }
}

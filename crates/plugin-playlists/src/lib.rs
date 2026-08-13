use retrotools_core::Game;
use retrotools_plugin_api::{Plugin, PluginContext, PluginOutcome, PluginResult};
use serde::Serialize;
use std::path::PathBuf;

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn sanitize_component(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c => c,
        })
        .collect()
}

fn selected_games<'a>(gameset: &'a retrotools_core::GameSet, kept: &[String]) -> Vec<&'a Game> {
    if kept.is_empty() {
        gameset.games.iter().collect()
    } else {
        gameset
            .games
            .iter()
            .filter(|g| kept.iter().any(|name| name == &g.name))
            .collect()
    }
}

fn rom_path_for(game: &Game) -> String {
    game.roms
        .first()
        .map(|r| r.name.clone())
        .unwrap_or_else(|| format!("{}.rom", game.name))
}

#[derive(Serialize)]
struct RetroArchPlaylist {
    version: &'static str,
    default_core_path: &'static str,
    default_core_name: &'static str,
    label_display_mode: u8,
    right_thumbnail_mode: u8,
    left_thumbnail_mode: u8,
    sort_mode: u8,
    items: Vec<RetroArchItem>,
}

#[derive(Serialize)]
struct RetroArchItem {
    path: String,
    label: String,
    core_path: &'static str,
    core_name: &'static str,
    crc32: String,
    db_name: String,
}

fn build_retroarch_playlist(platform: &str, games: &[&Game]) -> String {
    let db_name = format!("{platform}.lpl");
    let items = games
        .iter()
        .map(|game| {
            let crc = game
                .roms
                .first()
                .and_then(|r| r.crc32.clone())
                .unwrap_or_default();
            RetroArchItem {
                path: rom_path_for(game),
                label: game.name.clone(),
                core_path: "DETECT",
                core_name: "DETECT",
                crc32: format!("{}|crc", crc.to_uppercase()),
                db_name: db_name.clone(),
            }
        })
        .collect();

    let playlist = RetroArchPlaylist {
        version: "1.5",
        default_core_path: "",
        default_core_name: "",
        label_display_mode: 0,
        right_thumbnail_mode: 0,
        left_thumbnail_mode: 0,
        sort_mode: 0,
        items,
    };
    serde_json::to_string_pretty(&playlist).unwrap_or_default()
}

fn build_launchbox_xml(platform: &str, games: &[&Game]) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<LaunchBox>\n");
    for game in games {
        out.push_str("  <Game>\n");
        out.push_str(&format!("    <Title>{}</Title>\n", xml_escape(&game.name)));
        out.push_str(&format!(
            "    <ApplicationPath>{}</ApplicationPath>\n",
            xml_escape(&rom_path_for(game))
        ));
        out.push_str(&format!("    <Platform>{}</Platform>\n", xml_escape(platform)));
        out.push_str("  </Game>\n");
    }
    out.push_str("</LaunchBox>\n");
    out
}

fn build_esde_gamelist(games: &[&Game]) -> String {
    let mut out = String::from("<?xml version=\"1.0\"?>\n<gameList>\n");
    for game in games {
        out.push_str("  <game>\n");
        out.push_str(&format!("    <path>./{}</path>\n", xml_escape(&rom_path_for(game))));
        out.push_str(&format!("    <name>{}</name>\n", xml_escape(&game.name)));
        out.push_str("  </game>\n");
    }
    out.push_str("</gameList>\n");
    out
}

/// Generates emulator/frontend playlists (RetroArch `.lpl`, LaunchBox XML,
/// ES-DE `gamelist.xml`) from the current 1G1R selection, referencing ROMs
/// by the DAT's canonical file name (as produced by a build with
/// `rename_to_dat_name` enabled).
pub struct PlaylistPlugin;

impl Plugin for PlaylistPlugin {
    fn id(&self) -> &'static str {
        "playlists"
    }

    fn name(&self) -> &'static str {
        "Playlist Generator"
    }

    fn description(&self) -> &'static str {
        "Generates RetroArch (.lpl), LaunchBox (XML) and ES-DE (gamelist.xml) playlists from the current 1G1R selection."
    }

    fn run(&self, ctx: &PluginContext) -> PluginResult<PluginOutcome> {
        let games = selected_games(ctx.gameset, ctx.kept_game_names);
        if games.is_empty() {
            return Err("no games to include (empty DAT or empty 1G1R selection)".to_string());
        }

        std::fs::create_dir_all(ctx.output_dir).map_err(|e| e.to_string())?;

        let mut files_written = Vec::new();

        let lpl_path = ctx
            .output_dir
            .join(format!("{}.lpl", sanitize_component(&ctx.gameset.platform)));
        std::fs::write(&lpl_path, build_retroarch_playlist(&ctx.gameset.platform, &games))
            .map_err(|e| e.to_string())?;
        files_written.push(lpl_path);

        let launchbox_path = ctx
            .output_dir
            .join(format!("{}.xml", sanitize_component(&ctx.gameset.platform)));
        std::fs::write(&launchbox_path, build_launchbox_xml(&ctx.gameset.platform, &games))
            .map_err(|e| e.to_string())?;
        files_written.push(launchbox_path);

        let esde_path: PathBuf = ctx.output_dir.join("gamelist.xml");
        std::fs::write(&esde_path, build_esde_gamelist(&games)).map_err(|e| e.to_string())?;
        files_written.push(esde_path);

        Ok(PluginOutcome {
            summary: format!("{} game(s) written to 3 playlist format(s)", games.len()),
            files_written,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retrotools_core::dat::parse_dat_str;

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<datafile>
  <header><name>Test</name></header>
  <game name="Game A (Europe)">
    <rom name="Game A (Europe).bin" size="4" crc="deadbeef"/>
  </game>
  <game name="Game B (Europe)">
    <rom name="Game B (Europe).bin" size="4" crc="baadf00d"/>
  </game>
</datafile>"#;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rt26-plugin-playlists-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn writes_all_three_playlist_formats_for_kept_games() {
        let gameset = parse_dat_str(SAMPLE, "Nintendo - Game Boy").unwrap();
        let kept = vec!["Game A (Europe)".to_string()];
        let output_dir = temp_dir("basic");
        let ctx = PluginContext {
            gameset: &gameset,
            kept_game_names: &kept,
            source_dir: None,
            output_dir: &output_dir,
            dry_run: false,
        };

        let outcome = PlaylistPlugin.run(&ctx).unwrap();
        assert_eq!(outcome.files_written.len(), 3);
        for file in &outcome.files_written {
            assert!(file.exists());
        }

        let lpl = std::fs::read_to_string(&outcome.files_written[0]).unwrap();
        assert!(lpl.contains("Game A (Europe)"));
        assert!(!lpl.contains("Game B (Europe)"));
        assert!(lpl.contains("DEADBEEF|crc"));

        std::fs::remove_dir_all(&output_dir).ok();
    }

    #[test]
    fn falls_back_to_every_game_when_no_selection_given() {
        let gameset = parse_dat_str(SAMPLE, "Nintendo - Game Boy").unwrap();
        let kept = Vec::new();
        let output_dir = temp_dir("fallback");
        let ctx = PluginContext {
            gameset: &gameset,
            kept_game_names: &kept,
            source_dir: None,
            output_dir: &output_dir,
            dry_run: false,
        };

        let outcome = PlaylistPlugin.run(&ctx).unwrap();
        let gamelist = std::fs::read_to_string(&outcome.files_written[2]).unwrap();
        assert!(gamelist.contains("Game A (Europe)"));
        assert!(gamelist.contains("Game B (Europe)"));

        std::fs::remove_dir_all(&output_dir).ok();
    }
}

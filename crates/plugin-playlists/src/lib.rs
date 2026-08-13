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

// --- ES-DE custom collections (Phase 13) ------------------------------

fn collection_line_for(game: &Game) -> String {
    format!("./{}", rom_path_for(game))
}

/// Union-merges `new_lines` into `existing`'s lines: appends any line not
/// already present, in order, and never removes a line that's already
/// there. This is deliberately conservative — a collection only ever
/// grows from a re-run, so a line a user added by hand (or one that would
/// no longer be generated because the underlying set changed) is never
/// silently deleted.
fn merge_collection_lines(existing: &str, new_lines: &[String]) -> String {
    let mut lines: Vec<String> = existing.lines().map(|l| l.to_string()).collect();
    for line in new_lines {
        if !lines.iter().any(|l| l == line) {
            lines.push(line.clone());
        }
    }
    let mut out = lines.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

fn write_or_merge_collection(path: &std::path::Path, lines: &[String]) -> Result<bool, String> {
    if lines.is_empty() {
        return Ok(false);
    }
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let merged = merge_collection_lines(&existing, lines);
    if merged == existing {
        return Ok(false);
    }
    std::fs::write(path, merged).map_err(|e| e.to_string())?;
    Ok(true)
}

fn group_by<'a, K: Ord + Clone>(games: &[&'a Game], key: impl Fn(&Game) -> Option<K>) -> std::collections::BTreeMap<K, Vec<&'a Game>> {
    let mut groups: std::collections::BTreeMap<K, Vec<&Game>> = std::collections::BTreeMap::new();
    for game in games {
        if let Some(k) = key(game) {
            groups.entry(k).or_default().push(game);
        }
    }
    groups
}

/// Reads a `gamelist.xml` written by `retrotools-plugin-scraper` (same
/// block-oriented shape, so a light text scan is enough — no need for a
/// full XML parser for content this crate's own sibling plugin generates)
/// and returns, per ROM path, its genre and release year if the scraper
/// found them.
fn read_scraped_metadata(gamelist_path: &std::path::Path) -> std::collections::HashMap<String, (Option<String>, Option<String>)> {
    let mut out = std::collections::HashMap::new();
    let Ok(text) = std::fs::read_to_string(gamelist_path) else { return out };
    for block in text.split("<game>").skip(1) {
        let block = block.split("</game>").next().unwrap_or(block);
        let path = extract_tag(block, "path");
        let genre = extract_tag(block, "genre");
        let year = extract_tag(block, "releasedate");
        if let Some(path) = path {
            out.insert(path, (genre, year));
        }
    }
    out
}

fn extract_tag(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = block.find(&open)? + open.len();
    let end = block[start..].find(&close)? + start;
    Some(xml_unescape(&block[start..end]))
}

fn xml_unescape(value: &str) -> String {
    value.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"")
}

/// Generates EmulationStation/ES-DE "custom collection" `.cfg` files
/// (`collections/custom-<name>.cfg`, one ROM path per line — a much
/// simpler format than `gamelist.xml`) grouping the current 1G1R
/// selection by region and by language, and — when a `gamelist.xml`
/// written by `retrotools-plugin-scraper` is found in `output_dir` — also
/// by genre and by release year. Re-running only ever adds lines (see
/// `merge_collection_lines`), so collection entries added by hand outside
/// this tool are never lost.
pub struct CollectionsPlugin;

impl Plugin for CollectionsPlugin {
    fn id(&self) -> &'static str {
        "es-de-collections"
    }

    fn name(&self) -> &'static str {
        "ES-DE Custom Collections"
    }

    fn description(&self) -> &'static str {
        "Generate EmulationStation/ES-DE custom-collection files, grouped by region/language, \
         and by genre/year when scraped metadata is available."
    }

    fn run(&self, ctx: &PluginContext) -> PluginResult<PluginOutcome> {
        let games = selected_games(ctx.gameset, ctx.kept_game_names);
        if games.is_empty() {
            return Err("no games to include (empty DAT or empty 1G1R selection)".to_string());
        }

        let collections_dir = ctx.output_dir.join("collections");
        let gamelist_path = ctx.output_dir.join("gamelist.xml");
        let scraped = read_scraped_metadata(&gamelist_path);

        let mut plan: Vec<(String, Vec<String>)> = Vec::new();

        for (region, region_games) in group_by(&games, |g| g.regions.first().map(|r| r.0.clone())) {
            plan.push((
                format!("custom-by-region-{}.cfg", sanitize_component(&region).to_lowercase()),
                region_games.iter().map(|g| collection_line_for(g)).collect(),
            ));
        }
        for (language, lang_games) in group_by(&games, |g| g.languages.first().map(|l| l.0.clone())) {
            plan.push((
                format!("custom-by-language-{}.cfg", sanitize_component(&language).to_lowercase()),
                lang_games.iter().map(|g| collection_line_for(g)).collect(),
            ));
        }
        if !scraped.is_empty() {
            let genre_groups = group_by(&games, |g| scraped.get(&format!("./{}", rom_path_for(g))).and_then(|(genre, _)| genre.clone()));
            for (genre, genre_games) in genre_groups {
                plan.push((
                    format!("custom-by-genre-{}.cfg", sanitize_component(&genre).to_lowercase()),
                    genre_games.iter().map(|g| collection_line_for(g)).collect(),
                ));
            }
            let year_groups = group_by(&games, |g| scraped.get(&format!("./{}", rom_path_for(g))).and_then(|(_, year)| year.clone()));
            for (year, year_games) in year_groups {
                plan.push((format!("custom-by-year-{year}.cfg"), year_games.iter().map(|g| collection_line_for(g)).collect()));
            }
        }

        if ctx.dry_run {
            return Ok(PluginOutcome {
                summary: format!(
                    "[dry run] would write/update {} collection file(s) in '{}' ({} game(s) total; genre/year collections {})",
                    plan.len(),
                    collections_dir.display(),
                    games.len(),
                    if scraped.is_empty() { "unavailable — no gamelist.xml from the scraper found" } else { "available" }
                ),
                files_written: Vec::new(),
            });
        }

        std::fs::create_dir_all(&collections_dir).map_err(|e| e.to_string())?;
        let mut files_written = Vec::new();
        let mut updated = 0usize;
        for (filename, lines) in &plan {
            let path = collections_dir.join(filename);
            if write_or_merge_collection(&path, lines)? {
                updated += 1;
            }
            files_written.push(path);
        }

        Ok(PluginOutcome {
            summary: format!(
                "{} collection file(s) written/updated ({} regions/languages{})",
                updated,
                plan.len(),
                if scraped.is_empty() { "" } else { ", plus genre/year" }
            ),
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

    const REGIONS_SAMPLE: &str = r#"<?xml version="1.0"?>
<datafile>
  <header><name>Test</name></header>
  <game name="Game A (Europe)">
    <rom name="Game A (Europe).bin" size="4" crc="deadbeef"/>
  </game>
  <game name="Game B (USA)">
    <rom name="Game B (USA).bin" size="4" crc="baadf00d"/>
  </game>
</datafile>"#;

    #[test]
    fn writes_a_collection_file_per_region() {
        let gameset = parse_dat_str(REGIONS_SAMPLE, "Test System").unwrap();
        let output_dir = temp_dir("collections-region");
        let ctx = PluginContext {
            gameset: &gameset,
            kept_game_names: &[],
            source_dir: None,
            output_dir: &output_dir,
            dry_run: false,
        };

        let outcome = CollectionsPlugin.run(&ctx).unwrap();
        let europe = outcome.files_written.iter().find(|p| p.to_string_lossy().contains("europe")).unwrap();
        let usa = outcome.files_written.iter().find(|p| p.to_string_lossy().contains("usa")).unwrap();
        assert!(std::fs::read_to_string(europe).unwrap().contains("Game A (Europe).bin"));
        assert!(std::fs::read_to_string(usa).unwrap().contains("Game B (USA).bin"));

        std::fs::remove_dir_all(&output_dir).ok();
    }

    #[test]
    fn dry_run_reports_the_plan_without_writing_anything() {
        let gameset = parse_dat_str(REGIONS_SAMPLE, "Test System").unwrap();
        let output_dir = temp_dir("collections-dry-run");
        let ctx = PluginContext {
            gameset: &gameset,
            kept_game_names: &[],
            source_dir: None,
            output_dir: &output_dir,
            dry_run: true,
        };

        let outcome = CollectionsPlugin.run(&ctx).unwrap();
        assert!(outcome.summary.starts_with("[dry run]"));
        assert!(!output_dir.join("collections").exists());

        std::fs::remove_dir_all(&output_dir).ok();
    }

    #[test]
    fn regenerating_preserves_a_manually_added_line() {
        let gameset = parse_dat_str(REGIONS_SAMPLE, "Test System").unwrap();
        let output_dir = temp_dir("collections-preserve");
        let ctx = PluginContext {
            gameset: &gameset,
            kept_game_names: &[],
            source_dir: None,
            output_dir: &output_dir,
            dry_run: false,
        };
        CollectionsPlugin.run(&ctx).unwrap();

        let europe_path = output_dir.join("collections").join("custom-by-region-europe.cfg");
        let mut content = std::fs::read_to_string(&europe_path).unwrap();
        content.push_str("./manually-added-game.zip\n");
        std::fs::write(&europe_path, &content).unwrap();

        CollectionsPlugin.run(&ctx).unwrap();
        let after = std::fs::read_to_string(&europe_path).unwrap();
        assert!(after.contains("./manually-added-game.zip"), "manual entry must survive regeneration");
        assert!(after.contains("Game A (Europe).bin"));

        std::fs::remove_dir_all(&output_dir).ok();
    }

    #[test]
    fn genre_and_year_collections_use_scraper_written_gamelist_when_present() {
        let gameset = parse_dat_str(REGIONS_SAMPLE, "Test System").unwrap();
        let output_dir = temp_dir("collections-genre");
        std::fs::create_dir_all(&output_dir).unwrap();
        let gamelist = "<?xml version=\"1.0\"?>\n<gameList>\n  <game>\n    <path>./Game A (Europe).bin</path>\n    <name>Game A</name>\n    <genre>Platform</genre>\n    <releasedate>1991</releasedate>\n  </game>\n</gameList>\n";
        std::fs::write(output_dir.join("gamelist.xml"), gamelist).unwrap();

        let ctx = PluginContext {
            gameset: &gameset,
            kept_game_names: &[],
            source_dir: None,
            output_dir: &output_dir,
            dry_run: false,
        };
        let outcome = CollectionsPlugin.run(&ctx).unwrap();
        assert!(outcome.summary.contains("genre/year"));
        let genre_file = outcome.files_written.iter().find(|p| p.to_string_lossy().contains("by-genre-platform")).unwrap();
        assert!(std::fs::read_to_string(genre_file).unwrap().contains("Game A (Europe).bin"));
        let year_file = outcome.files_written.iter().find(|p| p.to_string_lossy().contains("by-year-1991")).unwrap();
        assert!(std::fs::read_to_string(year_file).unwrap().contains("Game A (Europe).bin"));

        std::fs::remove_dir_all(&output_dir).ok();
    }

    #[test]
    fn no_gamelist_means_no_genre_year_collections() {
        let gameset = parse_dat_str(REGIONS_SAMPLE, "Test System").unwrap();
        let output_dir = temp_dir("collections-no-gamelist");
        let ctx = PluginContext {
            gameset: &gameset,
            kept_game_names: &[],
            source_dir: None,
            output_dir: &output_dir,
            dry_run: false,
        };
        let outcome = CollectionsPlugin.run(&ctx).unwrap();
        assert!(!outcome.files_written.iter().any(|p| p.to_string_lossy().contains("by-genre")));

        std::fs::remove_dir_all(&output_dir).ok();
    }
}

mod cache;
mod client;
mod console_map;
mod rate_limit;

pub use cache::{load_hash_cache, save_hash_cache};
pub use client::{Credentials, RetroAchievementsClient};
pub use console_map::{default_console_table, load_or_default_table, ConsoleTable};
pub use rate_limit::RateLimiter;

use retrotools_common::config::RetroAchievementsCredentials;
use retrotools_plugin_api::{Plugin, PluginContext, PluginOutcome, PluginResult};
use std::collections::HashSet;
use std::time::Duration;

fn decrypt_credentials(creds: &RetroAchievementsCredentials) -> Result<Credentials, String> {
    use retrotools_common::secrets::decrypt_from_base64;
    Ok(Credentials {
        username: creds.username.clone(),
        api_key: decrypt_from_base64(&creds.api_key_encrypted).map_err(|e| e.to_string())?,
    })
}

/// Same substring-heuristic classification as
/// `retrotools-plugin-scraper::is_transient` — `ureq`'s structured status
/// code doesn't survive being collapsed into a plain `String` error.
fn is_transient(message: &str) -> bool {
    message.contains("429") || message.contains("timed out") || message.contains("status 5")
}

/// Looks up the cached RetroAchievements-compatible hash set for a
/// platform, via the console-map table — an empty set (not an error) if
/// the platform isn't mapped or nothing has been synced yet. This is the
/// function `retrotools-cli`/the UI call to populate
/// `RulePriority::retroachievements_compatible_roms` before running 1G1R
/// selection with `prefer_retroachievements_compatible` turned on.
pub fn load_cached_hashes_for_platform(platform: &str) -> HashSet<String> {
    let table = load_or_default_table();
    match table.entries.get(platform) {
        Some(console_id) => load_hash_cache(*console_id),
        None => HashSet::new(),
    }
}

/// Fetches (rate-limited, with retry-on-transient-failure) the current
/// RetroAchievements hash list for the platform's console and caches it
/// locally, then cross-references the 1G1R selection (`ctx.kept_game_names`
/// — falling back to every game in the DAT when no preview has run, same
/// convention as the playlist/scraper plugins) against that set, flagging
/// any kept game whose DAT-declared ROM MD5 doesn't match a known
/// RetroAchievements hash. This only *reports*; actually preferring an
/// RA-compatible alternate during 1G1R selection is the separate, opt-in
/// `RulePriority::prefer_retroachievements_compatible` tie-breaker (see
/// `retrotools_core::rules`), which reads the same cache via
/// `load_cached_hashes_for_platform` — this plugin doesn't touch selection
/// itself, it only keeps the cache fresh and reports on it.
pub struct RetroAchievementsPlugin;

impl Plugin for RetroAchievementsPlugin {
    fn id(&self) -> &'static str {
        "retroachievements-sync"
    }

    fn name(&self) -> &'static str {
        "RetroAchievements Compatibility Sync"
    }

    fn description(&self) -> &'static str {
        "Sync known RetroAchievements-compatible hashes for this platform and flag 1G1R selections with no known hash. \
         Needs a RetroAchievements username/API key configured in Settings first."
    }

    fn run(&self, ctx: &PluginContext) -> PluginResult<PluginOutcome> {
        let config = retrotools_common::config::AppConfig::load().map_err(|e| e.to_string())?;
        if !config.retroachievements.is_configured() {
            return Err(
                "RetroAchievements credentials aren't configured yet — add your username and API key in Settings first (nothing is synced without them)".to_string(),
            );
        }
        let credentials = decrypt_credentials(&config.retroachievements)?;

        let table = load_or_default_table();
        let Some(console_id) = table.entries.get(&ctx.gameset.platform).copied() else {
            return Err(format!(
                "no RetroAchievements console id mapped for platform '{}' — add one to the console-map.json in Settings/data, or accept it stays unmapped",
                ctx.gameset.platform
            ));
        };

        let target_names: Vec<&str> = if ctx.kept_game_names.is_empty() {
            ctx.gameset.games.iter().map(|g| g.name.as_str()).collect()
        } else {
            ctx.kept_game_names.iter().map(|s| s.as_str()).collect()
        };
        let targets: Vec<&retrotools_core::Game> = ctx.gameset.games.iter().filter(|g| target_names.contains(&g.name.as_str())).collect();
        if targets.is_empty() {
            return Err("no games to check (empty DAT or 1G1R selection)".to_string());
        }

        if ctx.dry_run {
            return Ok(PluginOutcome {
                summary: format!(
                    "[dry run] would sync RetroAchievements hashes for console {console_id} and cross-reference {} game(s)",
                    targets.len()
                ),
                files_written: Vec::new(),
            });
        }

        let client = RetroAchievementsClient::new();
        let limiter = RateLimiter::new(Duration::from_millis(1000), 3);
        let hashes = limiter.run(|| {
            client.fetch_console_hashes(&credentials, console_id).map_err(|(message, transient)| {
                let extra_transient = is_transient(&message);
                (message, transient || extra_transient)
            })
        })?;

        cache::save_hash_cache(console_id, &hashes)?;

        let mut unknown = Vec::new();
        for game in &targets {
            let known = game.roms.iter().any(|rom| rom.md5.as_deref().map(|md5| hashes.contains(&md5.to_lowercase())).unwrap_or(false));
            if !known {
                unknown.push(game.name.clone());
            }
        }

        let report_path = ctx.output_dir.join("retroachievements-report.txt");
        std::fs::create_dir_all(ctx.output_dir).map_err(|e| e.to_string())?;
        let mut report_text = format!("console {console_id}: {} known hash(es) cached\n\n", hashes.len());
        for name in &unknown {
            report_text.push_str(&format!("{name}\tno known RetroAchievements hash\n"));
        }
        std::fs::write(&report_path, &report_text).map_err(|e| e.to_string())?;

        Ok(PluginOutcome {
            summary: format!(
                "synced {} known hash(es) for console {console_id}; {} of {} selected game(s) have no known RetroAchievements hash",
                hashes.len(),
                unknown.len(),
                targets.len()
            ),
            files_written: vec![report_path],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retrotools_core::{DatHeader, DatType, Game, GameSet, Language, Region, RomFile};

    fn gameset_with_one_game(md5: Option<&str>) -> GameSet {
        GameSet {
            platform: "Test System".into(),
            dat_name: "Test".into(),
            dat_version: "1".into(),
            dat_type: DatType::Custom,
            header: DatHeader::default(),
            games: vec![Game {
                id: "1".into(),
                name: "Super Test Game".into(),
                platform: "Test System".into(),
                regions: vec![Region("Europe".into())],
                languages: vec![Language("En".into())],
                roms: vec![RomFile {
                    name: "Super Test Game.bin".into(),
                    size: 4,
                    crc32: None,
                    md5: md5.map(|s| s.to_string()),
                    sha1: None,
                    sha256: None,
                }],
                clone_of: None,
                rom_of: None,
                is_beta: false,
                is_proto: false,
                is_demo: false,
                is_sample: false,
                is_kiosk: false,
                is_promo: false,
                is_unlicensed: false,
                is_pirate: false,
                is_bad_dump: false,
                is_alt: false,
                revision: None,
            }],
        }
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("rt26-plugin-retroachievements-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn refuses_to_run_without_configured_credentials() {
        let gameset = gameset_with_one_game(Some("aaaa"));
        let output = temp_dir("no-creds-output");
        let ctx = PluginContext {
            gameset: &gameset,
            kept_game_names: &[],
            source_dir: None,
            output_dir: &output,
            match_report: None,
            dry_run: false,
        };
        let err = RetroAchievementsPlugin.run(&ctx).unwrap_err();
        assert!(err.contains("Settings"), "error should point the user at Settings, got: {err}");
        std::fs::remove_dir_all(&output).ok();
    }

    #[test]
    fn is_transient_classifies_rate_limit_and_server_errors_as_retryable() {
        assert!(is_transient("RetroAchievements request failed with status 429"));
        assert!(is_transient("request timed out"));
        assert!(!is_transient("RetroAchievements request failed with status 401"));
    }

    #[test]
    fn load_cached_hashes_for_unmapped_platform_is_an_empty_set() {
        let hashes = load_cached_hashes_for_platform("Some Totally Unmapped Platform XYZ");
        assert!(hashes.is_empty());
    }
}

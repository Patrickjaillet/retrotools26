mod cache;
mod client;
mod gamelist;
mod rate_limit;

pub use cache::{cache_size, download_if_missing, purge_to_limit};
pub use client::{Credentials, GameMedia, ScreenScraperClient};
pub use gamelist::{merge_entry, GamelistEntry};
pub use rate_limit::RateLimiter;

use retrotools_common::config::ScreenScraperCredentials;
use retrotools_plugin_api::{Plugin, PluginContext, PluginOutcome, PluginResult};
use std::path::PathBuf;
use std::time::Duration;

fn decrypt_credentials(creds: &ScreenScraperCredentials) -> Result<Credentials, String> {
    use retrotools_common::secrets::decrypt_from_base64;
    Ok(Credentials {
        dev_id: decrypt_from_base64(&creds.dev_id_encrypted).map_err(|e| e.to_string())?,
        dev_password: decrypt_from_base64(&creds.dev_password_encrypted)
            .map_err(|e| e.to_string())?,
        software_name: creds.software_name.clone(),
        user_id: decrypt_from_base64(&creds.user_id_encrypted).map_err(|e| e.to_string())?,
        user_password: decrypt_from_base64(&creds.user_password_encrypted)
            .map_err(|e| e.to_string())?,
    })
}

/// Best-effort classification of an HTTP-flavored error message into
/// "worth retrying" (rate-limited/server error) vs. "permanent" (bad
/// request/auth failure) — `ureq`'s error type doesn't survive being
/// collapsed to a `String` with its status code intact, so this is a
/// substring heuristic rather than a structured check.
fn is_transient(message: &str) -> bool {
    message.contains("429") || message.contains("timed out") || message.contains("Status(5")
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

pub struct ScraperPlugin;

impl Plugin for ScraperPlugin {
    fn id(&self) -> &'static str {
        "scraper"
    }

    fn name(&self) -> &'static str {
        "Metadata Scraper (ScreenScraper.fr)"
    }

    fn description(&self) -> &'static str {
        "Download box art, screenshots, videos and wheel logos from ScreenScraper.fr and write a gamelist.xml. \
         Needs ScreenScraper credentials configured in Settings first."
    }

    fn run(&self, ctx: &PluginContext) -> PluginResult<PluginOutcome> {
        let config = retrotools_common::config::AppConfig::load().map_err(|e| e.to_string())?;
        if !config.screenscraper.is_configured() {
            return Err(
                "ScreenScraper credentials aren't configured yet — add your developer ID/password and account ID/password in Settings first (nothing is scraped without them)".to_string(),
            );
        }
        let credentials = decrypt_credentials(&config.screenscraper)?;

        let target_names: Vec<&str> = if ctx.kept_game_names.is_empty() {
            ctx.gameset.games.iter().map(|g| g.name.as_str()).collect()
        } else {
            ctx.kept_game_names.iter().map(|s| s.as_str()).collect()
        };
        let targets: Vec<&retrotools_core::Game> = ctx
            .gameset
            .games
            .iter()
            .filter(|g| target_names.contains(&g.name.as_str()))
            .collect();
        if targets.is_empty() {
            return Err("no games to scrape (empty DAT or 1G1R selection)".to_string());
        }

        let platform_slug = slugify(&ctx.gameset.platform);
        let media_dir = ctx.output_dir.join("media").join(&platform_slug);
        let gamelist_path = ctx.output_dir.join("gamelist.xml");

        if ctx.dry_run {
            let with_hash = targets
                .iter()
                .filter(|g| g.roms.first().and_then(|r| r.crc32.as_ref()).is_some())
                .count();
            return Ok(PluginOutcome {
                summary: format!(
                    "[dry run] would look up {} game(s) on ScreenScraper.fr ({} have a usable hash), writing media into '{}' and updating '{}'",
                    targets.len(),
                    with_hash,
                    media_dir.display(),
                    gamelist_path.display()
                ),
                files_written: Vec::new(),
            });
        }

        let client = ScreenScraperClient::new();
        let limiter = RateLimiter::new(Duration::from_millis(1100), 3);
        let mut files_written = Vec::new();
        let mut gamelist_text = std::fs::read_to_string(&gamelist_path).unwrap_or_default();
        let mut found = 0usize;
        let mut not_found = 0usize;
        let mut skipped_no_hash = 0usize;
        let mut errors = Vec::new();

        for game in &targets {
            let Some(crc32) = game.roms.first().and_then(|r| r.crc32.clone()) else {
                skipped_no_hash += 1;
                continue;
            };

            let media = limiter.run(|| {
                client
                    .fetch_game_media(&credentials, &crc32)
                    .map_err(|message| {
                        let transient = is_transient(&message);
                        (message, transient)
                    })
            });

            let media = match media {
                Ok(media) => media,
                Err(message) => {
                    errors.push(format!("{}: {message}", game.name));
                    continue;
                }
            };
            let Some(media) = media else {
                not_found += 1;
                continue;
            };
            found += 1;

            let game_slug = slugify(&game.name);
            let mut entry = GamelistEntry {
                rom_path: format!(
                    "./{}",
                    game.roms
                        .first()
                        .map(|r| r.name.as_str())
                        .unwrap_or(&game.name)
                ),
                name: media.name.clone().unwrap_or_else(|| game.name.clone()),
                genre: media.genre.clone(),
                release_year: media.release_year.clone(),
                ..Default::default()
            };

            for (kind, url) in media.media_urls() {
                let extension = url
                    .rsplit('.')
                    .next()
                    .filter(|e| e.len() <= 4)
                    .unwrap_or("bin");
                let dest = media_dir.join(format!("{game_slug}-{kind}.{extension}"));
                match cache::download_if_missing(url, &dest) {
                    Ok(_) => {
                        files_written.push(dest.clone());
                        let relative =
                            format!("./media/{platform_slug}/{game_slug}-{kind}.{extension}");
                        match kind {
                            "box-art" => entry.image = Some(relative),
                            "video" => entry.video = Some(relative),
                            "wheel" => entry.marquee = Some(relative),
                            _ => {}
                        }
                    }
                    Err(err) => errors.push(format!("{} ({kind}): {err}", game.name)),
                }
            }

            gamelist_text = gamelist::merge_entry(&gamelist_text, &entry);
        }

        std::fs::write(&gamelist_path, &gamelist_text).map_err(|e| e.to_string())?;
        files_written.push(gamelist_path);

        let cache_root = ctx.output_dir.join("media");
        let limit_bytes = config.scraper_cache_limit_mb.saturating_mul(1024 * 1024);
        let purged = cache::purge_to_limit(&cache_root, limit_bytes).unwrap_or(0);

        let mut summary = format!("scraped {found} game(s), {not_found} not found on ScreenScraper, {skipped_no_hash} skipped (no hash)");
        if purged > 0 {
            summary.push_str(&format!(
                ", purged {purged} old cached media file(s) to stay under the {}MB limit",
                config.scraper_cache_limit_mb
            ));
        }
        if !errors.is_empty() {
            summary.push_str(&format!(
                " — {} error(s): {}",
                errors.len(),
                errors.join("; ")
            ));
        }

        Ok(PluginOutcome {
            summary,
            files_written,
        })
    }
}

pub fn media_cache_dir_for(output_dir: &std::path::Path, platform: &str) -> PathBuf {
    output_dir.join("media").join(slugify(platform))
}

#[cfg(test)]
mod tests {
    use super::*;
    use retrotools_core::{DatHeader, DatType, Game, GameSet, Language, Region, RomFile};

    fn gameset_with_one_game(crc32: Option<&str>) -> GameSet {
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
                    crc32: crc32.map(|s| s.to_string()),
                    md5: None,
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

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rt26-plugin-scraper-test-{name}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn refuses_to_run_without_configured_credentials() {
        let gameset = gameset_with_one_game(Some("12345678"));
        let output = temp_dir("no-creds-output");
        let ctx = PluginContext {
            gameset: &gameset,
            kept_game_names: &[],
            source_dir: None,
            output_dir: &output,
            match_report: None,
            dry_run: false,
        };
        let err = ScraperPlugin.run(&ctx).unwrap_err();
        assert!(
            err.contains("Settings"),
            "error should point the user at Settings, got: {err}"
        );
        std::fs::remove_dir_all(&output).ok();
    }

    #[test]
    fn is_transient_classifies_rate_limit_and_server_errors_as_retryable() {
        assert!(is_transient("ureq: http status: 429 Too Many Requests"));
        assert!(is_transient("Status(503, ..)"));
        assert!(is_transient("request timed out"));
        assert!(!is_transient("Status(400, ..): bad request"));
    }

    #[test]
    fn slugify_produces_filesystem_safe_names() {
        assert_eq!(slugify("Super Test Game!"), "super-test-game");
        assert_eq!(slugify("Sega - Mega Drive"), "sega-mega-drive");
    }

    #[test]
    fn media_cache_dir_is_scoped_per_platform() {
        let dir = media_cache_dir_for(std::path::Path::new("/out"), "Sega - Mega Drive");
        assert!(dir.ends_with("media/sega-mega-drive") || dir.ends_with("media\\sega-mega-drive"));
    }
}

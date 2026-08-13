use std::time::Duration;

/// Decrypted, ready-to-send credentials for one API call. Never persisted —
/// built fresh from `retrotools_common::config::ScreenScraperCredentials`
/// right before a call and dropped immediately after.
#[derive(Debug, Clone)]
pub struct Credentials {
    pub dev_id: String,
    pub dev_password: String,
    pub software_name: String,
    pub user_id: String,
    pub user_password: String,
}

#[derive(Debug, Clone, Default)]
pub struct GameMedia {
    pub name: Option<String>,
    pub box_art_url: Option<String>,
    pub screenshot_url: Option<String>,
    pub video_url: Option<String>,
    pub wheel_url: Option<String>,
    /// First genre reported by ScreenScraper, if any — feeds the dynamic
    /// genre-based collections in `retrotools-plugin-playlists` (Phase 13).
    pub genre: Option<String>,
    /// Release year as a plain 4-digit string (ScreenScraper reports full
    /// dates; only the year is kept, matching what a genre/year collection
    /// needs).
    pub release_year: Option<String>,
}

impl GameMedia {
    pub fn media_urls(&self) -> Vec<(&'static str, &str)> {
        [
            ("box-art", self.box_art_url.as_deref()),
            ("screenshot", self.screenshot_url.as_deref()),
            ("video", self.video_url.as_deref()),
            ("wheel", self.wheel_url.as_deref()),
        ]
        .into_iter()
        .filter_map(|(kind, url)| url.map(|u| (kind, u)))
        .collect()
    }
}

pub struct ScreenScraperClient {
    pub base_url: String,
}

impl ScreenScraperClient {
    pub fn new() -> Self {
        Self { base_url: "https://www.screenscraper.fr/api2".to_string() }
    }

    #[cfg(test)]
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self { base_url: base_url.into() }
    }

    /// Looks up a game by its CRC32 (already computed by `retrotools-core`
    /// during scanning/DAT parsing — no rehashing here). Returns
    /// `Ok(None)` when ScreenScraper genuinely has no match (HTTP 400/404
    /// from this endpoint means "not found", not an error worth retrying).
    pub fn fetch_game_media(&self, creds: &Credentials, crc32: &str) -> Result<Option<GameMedia>, String> {
        let url = format!(
            "{}/jeuInfos.php?output=json&devid={}&devpassword={}&softname={}&ssid={}&sspassword={}&crc={}",
            self.base_url,
            urlencode(&creds.dev_id),
            urlencode(&creds.dev_password),
            urlencode(&creds.software_name),
            urlencode(&creds.user_id),
            urlencode(&creds.user_password),
            urlencode(crc32),
        );

        let response = match ureq::get(&url).timeout(Duration::from_secs(20)).call() {
            Ok(response) => response,
            Err(ureq::Error::Status(400, _)) | Err(ureq::Error::Status(404, _)) => return Ok(None),
            Err(err) => return Err(format!("ScreenScraper request failed: {err}")),
        };

        let body = response.into_string().map_err(|e| e.to_string())?;
        let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| format!("malformed ScreenScraper response: {e}"))?;
        Ok(Some(parse_game_media(&json)))
    }
}

impl Default for ScreenScraperClient {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_game_media(json: &serde_json::Value) -> GameMedia {
    let jeu = &json["response"]["jeu"];

    let name = jeu["noms"]
        .as_array()
        .and_then(|names| names.first())
        .and_then(|n| n["text"].as_str())
        .or_else(|| jeu["nom"].as_str())
        .map(|s| s.to_string());

    let mut media = GameMedia { name, ..Default::default() };
    if let Some(entries) = jeu["medias"].as_array() {
        for entry in entries {
            let Some(kind) = entry["type"].as_str() else { continue };
            let Some(url) = entry["url"].as_str() else { continue };
            match kind {
                "box-2D" | "box2d" | "box-3D" if media.box_art_url.is_none() => {
                    media.box_art_url = Some(url.to_string())
                }
                "ss" | "screenshot" if media.screenshot_url.is_none() => media.screenshot_url = Some(url.to_string()),
                "video" if media.video_url.is_none() => media.video_url = Some(url.to_string()),
                "wheel" if media.wheel_url.is_none() => media.wheel_url = Some(url.to_string()),
                _ => {}
            }
        }
    }

    media.genre = jeu["genres"]
        .as_array()
        .and_then(|genres| genres.first())
        .and_then(|g| {
            g["noms"]
                .as_array()
                .and_then(|names| names.first())
                .and_then(|n| n["text"].as_str())
                .or_else(|| g["nomcourt"].as_str())
        })
        .map(|s| s.to_string());

    media.release_year = jeu["dates"]
        .as_array()
        .and_then(|dates| dates.first())
        .and_then(|d| d["text"].as_str())
        .and_then(|text| text.get(0..4))
        .filter(|year| year.chars().all(|c| c.is_ascii_digit()))
        .map(|s| s.to_string());

    media
}

fn urlencode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn serve_once(status_line: &str, body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let status_line = status_line.to_string();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        });
        format!("http://127.0.0.1:{port}")
    }

    fn creds() -> Credentials {
        Credentials {
            dev_id: "dev".into(),
            dev_password: "devpass".into(),
            software_name: "RetroTools2026".into(),
            user_id: "user".into(),
            user_password: "userpass".into(),
        }
    }

    #[test]
    fn parses_a_real_screenscraper_style_response_from_a_local_server() {
        let body = r#"{
            "response": {
                "jeu": {
                    "noms": [{"region": "wor", "text": "Super Test Game"}],
                    "medias": [
                        {"type": "box-2D", "region": "us", "url": "https://example.com/box.png"},
                        {"type": "ss", "region": "us", "url": "https://example.com/screenshot.png"},
                        {"type": "video", "url": "https://example.com/video.mp4"},
                        {"type": "wheel", "url": "https://example.com/wheel.png"}
                    ]
                }
            }
        }"#;
        let base_url = serve_once("200 OK", body);
        let client = ScreenScraperClient::with_base_url(base_url);
        let media = client.fetch_game_media(&creds(), "12345678").unwrap().unwrap();
        assert_eq!(media.name.as_deref(), Some("Super Test Game"));
        assert_eq!(media.box_art_url.as_deref(), Some("https://example.com/box.png"));
        assert_eq!(media.screenshot_url.as_deref(), Some("https://example.com/screenshot.png"));
        assert_eq!(media.video_url.as_deref(), Some("https://example.com/video.mp4"));
        assert_eq!(media.wheel_url.as_deref(), Some("https://example.com/wheel.png"));
    }

    #[test]
    fn parses_genre_and_release_year_when_present() {
        let body = r#"{
            "response": {
                "jeu": {
                    "noms": [{"region": "wor", "text": "Super Test Game"}],
                    "genres": [{"noms": [{"langue": "en", "text": "Platform"}]}],
                    "dates": [{"region": "us", "text": "1991-07-14"}],
                    "medias": []
                }
            }
        }"#;
        let base_url = serve_once("200 OK", body);
        let client = ScreenScraperClient::with_base_url(base_url);
        let media = client.fetch_game_media(&creds(), "12345678").unwrap().unwrap();
        assert_eq!(media.genre.as_deref(), Some("Platform"));
        assert_eq!(media.release_year.as_deref(), Some("1991"));
    }

    #[test]
    fn a_404_means_no_match_not_an_error() {
        let base_url = serve_once("404 Not Found", "{}");
        let client = ScreenScraperClient::with_base_url(base_url);
        assert!(client.fetch_game_media(&creds(), "deadbeef").unwrap().is_none());
    }

    #[test]
    fn missing_media_fields_are_handled_gracefully() {
        let body = r#"{"response": {"jeu": {"noms": [{"region": "wor", "text": "No Media Game"}], "medias": []}}}"#;
        let base_url = serve_once("200 OK", body);
        let client = ScreenScraperClient::with_base_url(base_url);
        let media = client.fetch_game_media(&creds(), "00000000").unwrap().unwrap();
        assert_eq!(media.name.as_deref(), Some("No Media Game"));
        assert!(media.media_urls().is_empty());
    }

    #[test]
    fn urlencode_escapes_reserved_characters() {
        assert_eq!(urlencode("a b&c"), "a%20b%26c");
        assert_eq!(urlencode("hello-world_1.0~"), "hello-world_1.0~");
    }
}

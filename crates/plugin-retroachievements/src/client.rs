use std::collections::HashSet;
use std::time::Duration;

/// Decrypted, ready-to-send credentials for one API call. Never persisted —
/// built fresh from `retrotools_common::config::RetroAchievementsCredentials`
/// right before a call and dropped immediately after (same pattern as
/// `retrotools-plugin-scraper::client::Credentials`).
#[derive(Debug, Clone)]
pub struct Credentials {
    pub username: String,
    pub api_key: String,
}

pub struct RetroAchievementsClient {
    pub base_url: String,
}

impl RetroAchievementsClient {
    pub fn new() -> Self {
        Self {
            base_url: "https://retroachievements.org/API".to_string(),
        }
    }

    #[cfg(test)]
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }

    /// Fetches every known hash for every game on one console
    /// (`API_GetGameList.php`, `h=1` requests the per-game hash list) and
    /// flattens the result into a single set — this project only needs "is
    /// this MD5 known-compatible", not a per-game breakdown, so a flat set
    /// is enough and much simpler to cache/consume than the nested
    /// title→hashes structure the API actually returns.
    pub fn fetch_console_hashes(
        &self,
        creds: &Credentials,
        console_id: u32,
    ) -> Result<HashSet<String>, (String, bool)> {
        let url = format!(
            "{}/API_GetGameList.php?z={}&y={}&i={}&h=1",
            self.base_url,
            urlencode(&creds.username),
            urlencode(&creds.api_key),
            console_id,
        );

        let response = ureq::get(&url)
            .timeout(Duration::from_secs(20))
            .call()
            .map_err(|err| classify_error(&err))?;
        let body = response.into_string().map_err(|e| (e.to_string(), false))?;
        let json: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| (format!("malformed RetroAchievements response: {e}"), false))?;
        Ok(parse_hashes(&json))
    }
}

impl Default for RetroAchievementsClient {
    fn default() -> Self {
        Self::new()
    }
}

fn classify_error(err: &ureq::Error) -> (String, bool) {
    match err {
        ureq::Error::Status(code, _) => {
            let transient = *code == 429 || *code >= 500;
            (
                format!("RetroAchievements request failed with status {code}"),
                transient,
            )
        }
        ureq::Error::Transport(transport) => (
            format!("RetroAchievements request failed: {transport}"),
            true,
        ),
    }
}

fn parse_hashes(json: &serde_json::Value) -> HashSet<String> {
    let Some(games) = json.as_array() else {
        return HashSet::new();
    };
    let mut hashes = HashSet::new();
    for game in games {
        if let Some(list) = game["Hashes"].as_array() {
            for hash in list {
                if let Some(text) = hash.as_str() {
                    hashes.insert(text.to_lowercase());
                }
            }
        }
    }
    hashes
}

fn urlencode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
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
            username: "player1".into(),
            api_key: "secret-key".into(),
        }
    }

    #[test]
    fn parses_and_flattens_hashes_from_a_real_style_response() {
        let body = r#"[
            {"Title": "Super Game", "ID": 1, "Hashes": ["AAAA1111", "bbbb2222"]},
            {"Title": "Other Game", "ID": 2, "Hashes": ["CCCC3333"]}
        ]"#;
        let base_url = serve_once("200 OK", body);
        let client = RetroAchievementsClient::with_base_url(base_url);
        let hashes = client.fetch_console_hashes(&creds(), 3).unwrap();
        assert_eq!(hashes.len(), 3);
        assert!(hashes.contains("aaaa1111"));
        assert!(hashes.contains("bbbb2222"));
        assert!(hashes.contains("cccc3333"));
    }

    #[test]
    fn a_game_with_no_hashes_field_is_skipped_without_error() {
        let body = r#"[{"Title": "No Hashes Game", "ID": 5}]"#;
        let base_url = serve_once("200 OK", body);
        let client = RetroAchievementsClient::with_base_url(base_url);
        let hashes = client.fetch_console_hashes(&creds(), 3).unwrap();
        assert!(hashes.is_empty());
    }

    #[test]
    fn a_429_is_classified_as_transient() {
        let base_url = serve_once("429 Too Many Requests", "{}");
        let client = RetroAchievementsClient::with_base_url(base_url);
        let (message, transient) = client.fetch_console_hashes(&creds(), 3).unwrap_err();
        assert!(message.contains("429"));
        assert!(transient);
    }

    #[test]
    fn a_401_is_classified_as_permanent() {
        let base_url = serve_once("401 Unauthorized", "{}");
        let client = RetroAchievementsClient::with_base_url(base_url);
        let (_, transient) = client.fetch_console_hashes(&creds(), 3).unwrap_err();
        assert!(!transient);
    }

    #[test]
    fn urlencode_escapes_reserved_characters() {
        assert_eq!(urlencode("a b&c"), "a%20b%26c");
    }
}

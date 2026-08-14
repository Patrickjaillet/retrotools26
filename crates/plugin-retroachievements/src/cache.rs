use std::collections::HashSet;
use std::path::PathBuf;

fn cache_dir() -> PathBuf {
    retrotools_common::config::plugin_data_dir_path("retroachievements")
        .map(|dir| dir.join("hash-cache"))
        .unwrap_or_else(|_| PathBuf::from("retroachievements-hash-cache"))
}

fn cache_file(console_id: u32) -> PathBuf {
    cache_dir().join(format!("{console_id}.json"))
}

/// Overwrites the cached hash set for one console — the API's hash list for
/// a console is a complete snapshot each time, not something to merge with
/// a previous partial fetch.
pub fn save_hash_cache(console_id: u32, hashes: &HashSet<String>) -> Result<(), String> {
    let path = cache_file(console_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut sorted: Vec<&String> = hashes.iter().collect();
    sorted.sort();
    let json = serde_json::to_string_pretty(&sorted).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

/// Returns an empty set (not an error) when nothing has been cached yet for
/// this console — "never synced" and "synced, found nothing" both mean "no
/// known hashes to cross-reference against" to a caller.
pub fn load_hash_cache(console_id: u32) -> HashSet<String> {
    let Ok(raw) = std::fs::read_to_string(cache_file(console_id)) else {
        return HashSet::new();
    };
    serde_json::from_str::<Vec<String>>(&raw)
        .map(|v| v.into_iter().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_cache_round_trips() {
        let mut hashes = HashSet::new();
        hashes.insert("aaaa".to_string());
        hashes.insert("bbbb".to_string());
        // Uses a console id unlikely to collide with a real cache file left
        // by another test/run on this machine.
        let console_id = 900_001 + (std::process::id() % 1000);
        save_hash_cache(console_id, &hashes).unwrap();
        let loaded = load_hash_cache(console_id);
        assert_eq!(loaded, hashes);
        std::fs::remove_file(cache_file(console_id)).ok();
    }

    #[test]
    fn loading_an_unsynced_console_returns_an_empty_set() {
        let loaded = load_hash_cache(900_999);
        assert!(loaded.is_empty());
    }
}

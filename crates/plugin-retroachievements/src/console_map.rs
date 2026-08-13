use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Platform (DAT/No-Intro name) → RetroAchievements console id. Not
/// exhaustive — same tradeoff as `retrotools-plugin-batocera-export`'s
/// system table: a small built-in default, written out as editable JSON on
/// first use, with an unmapped platform reported clearly rather than
/// guessed at.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConsoleTable {
    pub entries: BTreeMap<String, u32>,
}

pub fn default_console_table() -> ConsoleTable {
    let mut entries = BTreeMap::new();
    entries.insert("Nintendo - Nintendo Entertainment System".to_string(), 7);
    entries.insert("Nintendo - Super Nintendo Entertainment System".to_string(), 3);
    entries.insert("Nintendo - Game Boy".to_string(), 4);
    entries.insert("Nintendo - Game Boy Color".to_string(), 6);
    entries.insert("Nintendo - Game Boy Advance".to_string(), 5);
    entries.insert("Sega - Mega Drive - Genesis".to_string(), 1);
    entries.insert("Sega - Master System - Mark III".to_string(), 11);
    entries.insert("Sega - Game Gear".to_string(), 15);
    entries.insert("Sony - PlayStation".to_string(), 12);
    ConsoleTable { entries }
}

fn table_path() -> PathBuf {
    retrotools_common::config::plugin_data_dir_path("retroachievements")
        .map(|dir| dir.join("console-map.json"))
        .unwrap_or_else(|_| PathBuf::from("console-map.json"))
}

pub fn load_or_default_table() -> ConsoleTable {
    load_or_default_table_from(&table_path())
}

fn load_or_default_table_from(path: &std::path::Path) -> ConsoleTable {
    if let Ok(raw) = std::fs::read_to_string(path) {
        if let Ok(table) = serde_json::from_str(&raw) {
            return table;
        }
    }
    let table = default_console_table();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if let Ok(json) = serde_json::to_string_pretty(&table) {
        std::fs::write(path, json).ok();
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_table_covers_the_common_platforms() {
        let table = default_console_table();
        assert_eq!(table.entries.get("Sony - PlayStation"), Some(&12));
        assert_eq!(table.entries.get("Nintendo - Super Nintendo Entertainment System"), Some(&3));
    }

    #[test]
    fn loading_a_missing_file_seeds_and_returns_the_default() {
        let dir = std::env::temp_dir().join(format!("rt26-plugin-ra-consolemap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("console-map.json");
        let table = load_or_default_table_from(&path);
        assert_eq!(table.entries, default_console_table().entries);
        assert!(path.is_file());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn loading_an_existing_edited_file_respects_user_changes() {
        let dir = std::env::temp_dir().join(format!("rt26-plugin-ra-consolemap-edited-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("console-map.json");
        let mut custom = ConsoleTable::default();
        custom.entries.insert("Some Custom Platform".to_string(), 999);
        std::fs::write(&path, serde_json::to_string_pretty(&custom).unwrap()).unwrap();

        let loaded = load_or_default_table_from(&path);
        assert_eq!(loaded.entries.get("Some Custom Platform"), Some(&999));
        assert!(!loaded.entries.contains_key("Sony - PlayStation"));
        std::fs::remove_dir_all(&dir).ok();
    }
}

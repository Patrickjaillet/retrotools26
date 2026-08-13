use crate::rules::RulePriority;
use retrotools_common::error::AppResult;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleProfile {
    pub name: String,
    pub platform: Option<String>,
    pub rules: RulePriority,
}

impl RuleProfile {
    pub fn new(name: impl Into<String>, platform: Option<String>, rules: RulePriority) -> Self {
        Self {
            name: name.into(),
            platform,
            rules,
        }
    }
}

fn slugify(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

/// Persists reusable 1G1R rule profiles as JSON files on disk (one file per
/// profile), so a set of region/language/exclusion preferences can be saved
/// once per platform and applied again later.
pub struct ProfileStore {
    dir: PathBuf,
}

impl ProfileStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path_for(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{}.json", slugify(name)))
    }

    pub fn save(&self, profile: &RuleProfile) -> AppResult<PathBuf> {
        std::fs::create_dir_all(&self.dir)?;
        let path = self.path_for(&profile.name);
        let json = serde_json::to_string_pretty(profile)?;
        std::fs::write(&path, json)?;
        Ok(path)
    }

    pub fn load(&self, name: &str) -> AppResult<RuleProfile> {
        let raw = std::fs::read_to_string(self.path_for(name))?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn list(&self) -> AppResult<Vec<RuleProfile>> {
        if !self.dir.exists() {
            return Ok(Vec::new());
        }
        let mut profiles = Vec::new();
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let raw = std::fs::read_to_string(&path)?;
                if let Ok(profile) = serde_json::from_str::<RuleProfile>(&raw) {
                    profiles.push(profile);
                }
            }
        }
        profiles.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(profiles)
    }

    pub fn delete(&self, name: &str) -> AppResult<()> {
        let path = self.path_for(name);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}

/// Built-in profiles offered out of the box, matching common collector
/// presets referenced in the roadmap.
pub fn built_in_presets() -> Vec<RuleProfile> {
    vec![
        RuleProfile::new("Standard Europe", None, RulePriority::standard_europe()),
        RuleProfile::new("Standard USA", None, RulePriority::standard_usa()),
        RuleProfile::new(
            "Complete - No Filter",
            None,
            RulePriority::complete_no_filter(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rt26-profiles-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn saves_and_loads_a_profile_roundtrip() {
        let dir = temp_dir("roundtrip");
        let store = ProfileStore::new(&dir);
        let profile = RuleProfile::new(
            "My SNES Profile",
            Some("Super Nintendo".into()),
            RulePriority::standard_usa(),
        );
        store.save(&profile).unwrap();

        let loaded = store.load("My SNES Profile").unwrap();
        assert_eq!(loaded.name, profile.name);
        assert_eq!(loaded.platform, profile.platform);
        assert_eq!(loaded.rules.region_order, profile.rules.region_order);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn lists_saved_profiles_sorted_by_name() {
        let dir = temp_dir("list");
        let store = ProfileStore::new(&dir);
        store
            .save(&RuleProfile::new("Zeta", None, RulePriority::default()))
            .unwrap();
        store
            .save(&RuleProfile::new("Alpha", None, RulePriority::default()))
            .unwrap();

        let profiles = store.list().unwrap();
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].name, "Alpha");
        assert_eq!(profiles[1].name, "Zeta");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn deletes_a_profile() {
        let dir = temp_dir("delete");
        let store = ProfileStore::new(&dir);
        let profile = RuleProfile::new("Temp", None, RulePriority::default());
        store.save(&profile).unwrap();
        assert_eq!(store.list().unwrap().len(), 1);

        store.delete("Temp").unwrap();
        assert_eq!(store.list().unwrap().len(), 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn built_in_presets_are_named_after_the_roadmap() {
        let names: Vec<_> = built_in_presets().into_iter().map(|p| p.name).collect();
        assert!(names.contains(&"Standard Europe".to_string()));
        assert!(names.contains(&"Standard USA".to_string()));
        assert!(names.contains(&"Complete - No Filter".to_string()));
    }
}

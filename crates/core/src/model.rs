use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DatType {
    NoIntro,
    Redump,
    Tosec,
    Mame,
    Custom,
}

impl std::fmt::Display for DatType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            DatType::NoIntro => "No-Intro",
            DatType::Redump => "Redump",
            DatType::Tosec => "TOSEC",
            DatType::Mame => "MAME",
            DatType::Custom => "Custom",
        };
        write!(f, "{label}")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DatHeader {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub homepage: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Region(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Language(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RomFile {
    pub name: String,
    pub size: u64,
    pub crc32: Option<String>,
    pub md5: Option<String>,
    pub sha1: Option<String>,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub regions: Vec<Region>,
    pub languages: Vec<Language>,
    pub roms: Vec<RomFile>,
    pub clone_of: Option<String>,
    pub rom_of: Option<String>,
    pub is_beta: bool,
    pub is_proto: bool,
    pub is_demo: bool,
    pub is_sample: bool,
    pub is_kiosk: bool,
    pub is_promo: bool,
    pub is_unlicensed: bool,
    pub is_pirate: bool,
    pub is_bad_dump: bool,
    pub is_alt: bool,
    pub revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSet {
    pub platform: String,
    pub dat_name: String,
    pub dat_version: String,
    pub dat_type: DatType,
    pub header: DatHeader,
    pub games: Vec<Game>,
}

impl GameSet {
    pub fn find_clones_of<'a>(&'a self, parent_id: &str) -> Vec<&'a Game> {
        self.games
            .iter()
            .filter(|game| game.clone_of.as_deref() == Some(parent_id))
            .collect()
    }

    pub fn is_parent(&self, game: &Game) -> bool {
        game.clone_of.is_none()
    }
}

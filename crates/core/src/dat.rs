use crate::model::{DatHeader, DatType, Game, GameSet, Language, Region, RomFile};
use quick_xml::events::Event;
use quick_xml::Reader;
use retrotools_common::error::{AppError, AppResult};
use std::io::Read;
use std::path::Path;

const KNOWN_REGIONS: &[&str] = &[
    "World",
    "Europe",
    "USA",
    "Japan",
    "Asia",
    "Australia",
    "Brazil",
    "Canada",
    "China",
    "Denmark",
    "Finland",
    "France",
    "Germany",
    "Greece",
    "Hong Kong",
    "India",
    "Ireland",
    "Israel",
    "Italy",
    "Korea",
    "Latin America",
    "Netherlands",
    "New Zealand",
    "Norway",
    "Poland",
    "Portugal",
    "Russia",
    "Scandinavia",
    "Spain",
    "Sweden",
    "Switzerland",
    "Taiwan",
    "Turkey",
    "UK",
    "United Kingdom",
    "USA, Europe",
];

const KNOWN_LANGUAGES: &[&str] = &[
    "En", "Fr", "De", "Es", "It", "Nl", "Sv", "Da", "No", "Fi", "Zh", "Ja", "Ko", "Pt", "Ru", "Pl",
    "Cs", "Hu", "Tr", "El", "Ar", "He", "Zh-Hans", "Zh-Hant",
];

#[derive(Debug, Default, Clone)]
struct NameTags {
    regions: Vec<Region>,
    languages: Vec<Language>,
    is_beta: bool,
    is_proto: bool,
    is_demo: bool,
    is_sample: bool,
    is_kiosk: bool,
    is_promo: bool,
    is_unlicensed: bool,
    is_pirate: bool,
    is_bad_dump: bool,
    is_alt: bool,
    revision: Option<String>,
}

fn extract_parens(name: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    for ch in name.chars() {
        match ch {
            '(' => {
                if depth == 0 {
                    current.clear();
                }
                depth += 1;
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    result.push(current.clone());
                }
            }
            _ if depth > 0 => current.push(ch),
            _ => {}
        }
    }
    result
}

fn parse_name_tags(name: &str) -> NameTags {
    let mut tags = NameTags::default();
    for group in extract_parens(name) {
        let lower = group.to_lowercase();
        if lower.contains("beta") {
            tags.is_beta = true;
        }
        if lower.contains("proto") {
            tags.is_proto = true;
        }
        if lower.contains("demo") {
            tags.is_demo = true;
        }
        if lower.contains("sample") {
            tags.is_sample = true;
        }
        if lower.contains("kiosk") {
            tags.is_kiosk = true;
        }
        if lower.contains("promo") {
            tags.is_promo = true;
        }
        if lower.contains("unl") {
            tags.is_unlicensed = true;
        }
        if lower.contains("pirate") {
            tags.is_pirate = true;
        }
        if lower == "alt" || lower.starts_with("alt ") {
            tags.is_alt = true;
        }
        if lower.starts_with("rev")
            || lower.starts_with('v') && lower.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
        {
            tags.revision = Some(group.clone());
        }

        let mut matched_region = false;
        for part in group.split(',').map(str::trim) {
            if let Some(region) = KNOWN_REGIONS.iter().find(|r| r.eq_ignore_ascii_case(part)) {
                tags.regions.push(Region((*region).to_string()));
                matched_region = true;
            }
        }
        if !matched_region {
            for part in group.split(',').map(str::trim) {
                if KNOWN_LANGUAGES.iter().any(|l| l.eq_ignore_ascii_case(part)) {
                    tags.languages.push(Language(part.to_string()));
                }
            }
        }
    }
    if name.to_lowercase().contains("[b]") || name.to_lowercase().contains("bad dump") {
        tags.is_bad_dump = true;
    }
    tags
}

pub fn detect_dat_type(header: &DatHeader, root_tag: &str) -> DatType {
    let haystack = format!(
        "{} {} {} {}",
        header.name, header.description, header.author, header.homepage
    )
    .to_lowercase();

    if root_tag.eq_ignore_ascii_case("mame") || haystack.contains("mame") {
        DatType::Mame
    } else if haystack.contains("no-intro") || haystack.contains("no intro") {
        DatType::NoIntro
    } else if haystack.contains("redump") {
        DatType::Redump
    } else if haystack.contains("tosec") {
        DatType::Tosec
    } else {
        DatType::Custom
    }
}

fn read_dat_source(path: &Path) -> AppResult<String> {
    let is_zip = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("zip"))
        .unwrap_or(false);

    if !is_zip {
        return std::fs::read_to_string(path).map_err(AppError::Io);
    }

    let file = std::fs::File::open(path).map_err(AppError::Io)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::DatParsing(format!("invalid ZIP archive: {e}")))?;

    let entry_name = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
        .find(|name| {
            let lower = name.to_lowercase();
            lower.ends_with(".dat") || lower.ends_with(".xml")
        })
        .ok_or_else(|| AppError::DatParsing("no .dat/.xml entry found inside ZIP".into()))?;

    let mut entry = archive
        .by_name(&entry_name)
        .map_err(|e| AppError::DatParsing(format!("cannot read ZIP entry: {e}")))?;
    let mut contents = String::new();
    entry.read_to_string(&mut contents).map_err(AppError::Io)?;
    Ok(contents)
}

fn attr_value(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if a.key.as_ref() == key.as_bytes() {
            String::from_utf8(a.value.to_vec()).ok()
        } else {
            None
        }
    })
}

fn parse_dat_xml(xml: &str, platform_hint: &str) -> AppResult<GameSet> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut header = DatHeader::default();
    let mut games: Vec<Game> = Vec::new();
    let mut root_tag = String::new();

    let mut in_header = false;
    let mut header_field: Option<String> = None;
    let mut current_game: Option<Game> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if root_tag.is_empty() && (tag == "datafile" || tag == "mame") {
                    root_tag = tag.clone();
                }
                match tag.as_str() {
                    "header" => in_header = true,
                    "game" | "machine" | "software" if !in_header => {
                        let name = attr_value(&e, "name").unwrap_or_default();
                        let clone_of = attr_value(&e, "cloneof");
                        let rom_of = attr_value(&e, "romof");
                        let tags = parse_name_tags(&name);
                        current_game = Some(Game {
                            id: name.clone(),
                            name,
                            platform: platform_hint.to_string(),
                            regions: tags.regions,
                            languages: tags.languages,
                            roms: Vec::new(),
                            clone_of,
                            rom_of,
                            is_beta: tags.is_beta,
                            is_proto: tags.is_proto,
                            is_demo: tags.is_demo,
                            is_sample: tags.is_sample,
                            is_kiosk: tags.is_kiosk,
                            is_promo: tags.is_promo,
                            is_unlicensed: tags.is_unlicensed,
                            is_pirate: tags.is_pirate,
                            is_bad_dump: tags.is_bad_dump,
                            is_alt: tags.is_alt,
                            revision: tags.revision,
                        });
                    }
                    "rom" if !in_header => {
                        if let Some(game) = current_game.as_mut() {
                            let rom = RomFile {
                                name: attr_value(&e, "name").unwrap_or_default(),
                                size: attr_value(&e, "size")
                                    .and_then(|s| s.parse::<u64>().ok())
                                    .unwrap_or(0),
                                crc32: attr_value(&e, "crc"),
                                md5: attr_value(&e, "md5"),
                                sha1: attr_value(&e, "sha1"),
                                sha256: attr_value(&e, "sha256"),
                            };
                            if attr_value(&e, "status").as_deref() == Some("baddump") {
                                game.is_bad_dump = true;
                            }
                            game.roms.push(rom);
                        }
                    }
                    _ if in_header => {
                        header_field = Some(tag);
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "rom" && !in_header {
                    if let Some(game) = current_game.as_mut() {
                        let rom = RomFile {
                            name: attr_value(&e, "name").unwrap_or_default(),
                            size: attr_value(&e, "size")
                                .and_then(|s| s.parse::<u64>().ok())
                                .unwrap_or(0),
                            crc32: attr_value(&e, "crc"),
                            md5: attr_value(&e, "md5"),
                            sha1: attr_value(&e, "sha1"),
                            sha256: attr_value(&e, "sha256"),
                        };
                        game.roms.push(rom);
                    }
                } else if (tag == "game" || tag == "machine") && !in_header {
                    let name = attr_value(&e, "name").unwrap_or_default();
                    let clone_of = attr_value(&e, "cloneof");
                    let rom_of = attr_value(&e, "romof");
                    let tags = parse_name_tags(&name);
                    games.push(Game {
                        id: name.clone(),
                        name,
                        platform: platform_hint.to_string(),
                        regions: tags.regions,
                        languages: tags.languages,
                        roms: Vec::new(),
                        clone_of,
                        rom_of,
                        is_beta: tags.is_beta,
                        is_proto: tags.is_proto,
                        is_demo: tags.is_demo,
                        is_sample: tags.is_sample,
                        is_kiosk: tags.is_kiosk,
                        is_promo: tags.is_promo,
                        is_unlicensed: tags.is_unlicensed,
                        is_pirate: tags.is_pirate,
                        is_bad_dump: tags.is_bad_dump,
                        is_alt: tags.is_alt,
                        revision: tags.revision,
                    });
                }
            }
            Ok(Event::Text(e)) => {
                if in_header {
                    if let Some(field) = header_field.take() {
                        let text = e.unescape().unwrap_or_default().trim().to_string();
                        match field.as_str() {
                            "name" => header.name = text,
                            "description" => header.description = text,
                            "version" => header.version = text,
                            "author" => header.author = text,
                            "homepage" => header.homepage = text,
                            "url" => header.url = text,
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::End(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "header" => in_header = false,
                    "game" | "machine" | "software" => {
                        if let Some(game) = current_game.take() {
                            games.push(game);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(AppError::DatParsing(format!(
                    "XML parse error at position {}: {e}",
                    reader.buffer_position()
                )))
            }
            _ => {}
        }
        buf.clear();
    }

    if games.is_empty() && header.name.is_empty() {
        return Err(AppError::DatParsing(
            "no <datafile>/<mame> root with games found".into(),
        ));
    }

    let dat_type = detect_dat_type(&header, &root_tag);
    let platform = if !platform_hint.is_empty() {
        platform_hint.to_string()
    } else if !header.description.is_empty() {
        header.description.clone()
    } else {
        header.name.clone()
    };

    Ok(GameSet {
        platform,
        dat_name: header.name.clone(),
        dat_version: header.version.clone(),
        dat_type,
        header,
        games,
    })
}

pub fn parse_dat_file(path: &Path) -> AppResult<GameSet> {
    let contents = read_dat_source(path)?;
    let platform_hint = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let gameset = parse_dat_xml(&contents, platform_hint)?;
    validate_dat_integrity(&gameset)?;
    Ok(gameset)
}

pub fn parse_dat_str(xml: &str, platform_hint: &str) -> AppResult<GameSet> {
    let gameset = parse_dat_xml(xml, platform_hint)?;
    validate_dat_integrity(&gameset)?;
    Ok(gameset)
}

pub fn validate_dat_integrity(gameset: &GameSet) -> AppResult<()> {
    if gameset.games.is_empty() {
        return Err(AppError::DatParsing(
            "DAT contains no games/machines".into(),
        ));
    }

    let known_ids: std::collections::HashSet<&str> =
        gameset.games.iter().map(|g| g.id.as_str()).collect();

    for game in &gameset.games {
        if game.name.trim().is_empty() {
            return Err(AppError::DatParsing("game with empty name found".into()));
        }
        if let Some(crc_len_err) = game.roms.iter().find_map(|rom| {
            rom.crc32
                .as_ref()
                .filter(|crc| crc.len() != 8 || !crc.chars().all(|c| c.is_ascii_hexdigit()))
                .map(|crc| format!("rom '{}' has invalid CRC32 '{}'", rom.name, crc))
        }) {
            return Err(AppError::DatParsing(crc_len_err));
        }
        if let Some(parent) = &game.clone_of {
            if !known_ids.contains(parent.as_str()) {
                tracing::warn!(
                    game = %game.name,
                    parent = %parent,
                    "clone references a parent not present in this DAT"
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NO_INTRO_SAMPLE: &str = r#"<?xml version="1.0"?>
<!DOCTYPE datafile PUBLIC "-//Logiqx//DTD ROM Management Datafile//EN" "http://www.logiqx.com/Dats/datafile.dtd">
<datafile>
  <header>
    <name>Nintendo - Game Boy</name>
    <description>Nintendo - Game Boy</description>
    <version>20260101</version>
    <author>No-Intro</author>
    <homepage>No-Intro</homepage>
    <url>https://no-intro.org</url>
  </header>
  <game name="Super Game (Europe)">
    <description>Super Game (Europe)</description>
    <rom name="Super Game (Europe).gb" size="32768" crc="1A2B3C4D" md5="d41d8cd98f00b204e9800998ecf8427e" sha1="da39a3ee5e6b4b0d3255bfef95601890afd80709"/>
  </game>
  <game name="Super Game (USA)" cloneof="Super Game (Europe)">
    <description>Super Game (USA)</description>
    <rom name="Super Game (USA).gb" size="32768" crc="5E6F7A8B"/>
  </game>
  <game name="Super Game (Japan) (Beta)" cloneof="Super Game (Europe)">
    <description>Super Game (Japan) (Beta)</description>
    <rom name="Super Game (Japan) (Beta).gb" size="32768" crc="9C0D1E2F"/>
  </game>
</datafile>"#;

    const MAME_SAMPLE: &str = r#"<?xml version="1.0"?>
<mame build="0.260">
  <machine name="pacman" sourcefile="pacman.cpp">
    <description>Pac-Man (Midway)</description>
    <rom name="pacman.6e" size="4096" crc="c1e6ab10"/>
  </machine>
  <machine name="pacmanf" cloneof="pacman" romof="pacman">
    <description>Pac-Man (Fast Shoot hack)</description>
    <rom name="pacmanf.6e" size="4096" crc="a2c1cf85"/>
  </machine>
</mame>"#;

    #[test]
    fn parses_no_intro_header_and_games() {
        let gameset = parse_dat_str(NO_INTRO_SAMPLE, "Nintendo - Game Boy").unwrap();
        assert_eq!(gameset.header.name, "Nintendo - Game Boy");
        assert_eq!(gameset.dat_version, "20260101");
        assert_eq!(gameset.dat_type, DatType::NoIntro);
        assert_eq!(gameset.games.len(), 3);
    }

    #[test]
    fn detects_clone_of_and_region_tags() {
        let gameset = parse_dat_str(NO_INTRO_SAMPLE, "Nintendo - Game Boy").unwrap();
        let usa = gameset
            .games
            .iter()
            .find(|g| g.name.contains("USA"))
            .unwrap();
        assert_eq!(usa.clone_of.as_deref(), Some("Super Game (Europe)"));
        assert_eq!(usa.regions, vec![Region("USA".to_string())]);

        let europe = gameset
            .games
            .iter()
            .find(|g| g.name.contains("Europe"))
            .unwrap();
        assert!(europe.clone_of.is_none());
        assert_eq!(europe.regions, vec![Region("Europe".to_string())]);
    }

    #[test]
    fn detects_beta_tag() {
        let gameset = parse_dat_str(NO_INTRO_SAMPLE, "Nintendo - Game Boy").unwrap();
        let beta = gameset
            .games
            .iter()
            .find(|g| g.name.contains("Beta"))
            .unwrap();
        assert!(beta.is_beta);
    }

    #[test]
    fn parses_mame_machines() {
        let gameset = parse_dat_str(MAME_SAMPLE, "").unwrap();
        assert_eq!(gameset.dat_type, DatType::Mame);
        assert_eq!(gameset.games.len(), 2);
        let clone = gameset.games.iter().find(|g| g.id == "pacmanf").unwrap();
        assert_eq!(clone.clone_of.as_deref(), Some("pacman"));
        assert_eq!(clone.rom_of.as_deref(), Some("pacman"));
    }

    #[test]
    fn rejects_empty_dat() {
        let empty =
            r#"<?xml version="1.0"?><datafile><header><name>Empty</name></header></datafile>"#;
        assert!(parse_dat_str(empty, "Empty").is_err());
    }

    #[test]
    fn rejects_invalid_crc() {
        let bad_crc = r#"<?xml version="1.0"?><datafile><header><name>Bad</name></header>
<game name="G"><rom name="g.bin" size="10" crc="ZZ"/></game></datafile>"#;
        assert!(parse_dat_str(bad_crc, "Bad").is_err());
    }

    #[test]
    fn reads_zip_wrapped_dat() {
        let dir = std::env::temp_dir().join(format!("rt26-dat-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let zip_path = dir.join("sample.zip");
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("sample.dat", zip::write::FileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut writer, NO_INTRO_SAMPLE.as_bytes()).unwrap();
        writer.finish().unwrap();

        let gameset = parse_dat_file(&zip_path).unwrap();
        assert_eq!(gameset.games.len(), 3);

        std::fs::remove_dir_all(&dir).ok();
    }
}

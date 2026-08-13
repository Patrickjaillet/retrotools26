/// One `<game>` entry to add/update in a `gamelist.xml`.
#[derive(Debug, Clone)]
pub struct GamelistEntry {
    pub rom_path: String,
    pub name: String,
    pub image: Option<String>,
    pub video: Option<String>,
    pub marquee: Option<String>,
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn entry_fragment(entry: &GamelistEntry) -> String {
    let mut fragment = String::new();
    fragment.push_str("  <game>\n");
    fragment.push_str(&format!("    <path>{}</path>\n", xml_escape(&entry.rom_path)));
    fragment.push_str(&format!("    <name>{}</name>\n", xml_escape(&entry.name)));
    if let Some(image) = &entry.image {
        fragment.push_str(&format!("    <image>{}</image>\n", xml_escape(image)));
    }
    if let Some(video) = &entry.video {
        fragment.push_str(&format!("    <video>{}</video>\n", xml_escape(video)));
    }
    if let Some(marquee) = &entry.marquee {
        fragment.push_str(&format!("    <marquee>{}</marquee>\n", xml_escape(marquee)));
    }
    fragment.push_str("  </game>\n");
    fragment
}

/// Replaces (or appends) the `<game>` block whose `<path>` matches
/// `entry.rom_path`, leaving every other game's entry — including ones a
/// human edited by hand — completely untouched. Same block-replace strategy
/// as the Batocera export plugin's `es_systems.cfg` merge.
pub fn merge_entry(existing: &str, entry: &GamelistEntry) -> String {
    let path_tag = format!("<path>{}</path>", xml_escape(&entry.rom_path));
    let fragment = entry_fragment(entry);
    let mut result = String::new();
    let rest;

    if let Some(path_pos) = existing.find(&path_tag) {
        let block_start = existing[..path_pos].rfind("<game>").unwrap_or(0);
        let block_end = existing[path_pos..]
            .find("</game>")
            .map(|i| path_pos + i + "</game>".len())
            .unwrap_or(existing.len());
        result.push_str(&existing[..block_start]);
        result.push_str(&fragment);
        rest = &existing[block_end..];
    } else if let Some(close_pos) = existing.rfind("</gameList>") {
        result.push_str(&existing[..close_pos]);
        result.push_str(&fragment);
        rest = &existing[close_pos..];
    } else {
        result.push_str("<?xml version=\"1.0\"?>\n<gameList>\n");
        result.push_str(&fragment);
        result.push_str("</gameList>\n");
        return result;
    }
    result.push_str(rest);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, name: &str) -> GamelistEntry {
        GamelistEntry {
            rom_path: path.to_string(),
            name: name.to_string(),
            image: Some(format!("media/{name}-box.png")),
            video: None,
            marquee: None,
        }
    }

    #[test]
    fn creates_a_fresh_gamelist_when_none_exists() {
        let result = merge_entry("", &entry("./game.zip", "Game A"));
        assert!(result.contains("<path>./game.zip</path>"));
        assert!(result.contains("<name>Game A</name>"));
        assert!(result.starts_with("<?xml"));
    }

    #[test]
    fn appends_a_second_game_without_disturbing_the_first() {
        let first = merge_entry("", &entry("./a.zip", "Game A"));
        let both = merge_entry(&first, &entry("./b.zip", "Game B"));
        assert!(both.contains("<name>Game A</name>"));
        assert!(both.contains("<name>Game B</name>"));
        assert_eq!(both.matches("<game>").count(), 2);
    }

    #[test]
    fn re_scraping_the_same_game_updates_in_place_without_duplicating() {
        let first = merge_entry("", &entry("./a.zip", "Game A"));
        let updated = merge_entry(&first, &entry("./a.zip", "Game A (Updated)"));
        assert_eq!(updated.matches("<game>").count(), 1);
        assert!(updated.contains("Game A (Updated)"));
        assert!(!updated.contains(">Game A<"));
    }

    #[test]
    fn a_hand_edited_entry_for_a_different_game_survives() {
        let hand_edited = "<?xml version=\"1.0\"?>\n<gameList>\n  <game>\n    <path>./manual.zip</path>\n    <name>Hand Edited</name>\n    <desc>My own notes</desc>\n  </game>\n</gameList>\n";
        let merged = merge_entry(hand_edited, &entry("./scraped.zip", "Scraped Game"));
        assert!(merged.contains("Hand Edited"));
        assert!(merged.contains("My own notes"));
        assert!(merged.contains("Scraped Game"));
    }

    #[test]
    fn xml_special_characters_are_escaped() {
        let result = merge_entry("", &entry("./a & b.zip", "Foo <Bar> \"Baz\""));
        assert!(result.contains("./a &amp; b.zip"));
        assert!(result.contains("Foo &lt;Bar&gt; &quot;Baz&quot;"));
    }
}

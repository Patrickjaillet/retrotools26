/// Procedurally generated per-platform badges (a colored circle with 2-4
/// letter initials), used wherever the UI lists platforms/systems. There is
/// no source of real system logos here on purpose: the DAT platform names
/// this app deals with are an open-ended set typed by whichever DAT the
/// user imports, and — separately — real console/company logos are
/// trademarks that can't be bundled in a public MIT-licensed repository
/// without the trademark holder's permission, regardless of this project's
/// own license. A deterministic, generated badge sidesteps both problems:
/// it works for any platform name and carries no licensing risk.
use egui::{Color32, Rounding, Ui};

fn fnv1a(name: &str) -> u32 {
    let mut hash: u32 = 2_166_136_261;
    for byte in name.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - (((h / 60.0) % 2.0) - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = match h as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r1 + m) * 255.0) as u8,
        ((g1 + m) * 255.0) as u8,
        ((b1 + m) * 255.0) as u8,
    )
}

/// A deterministic color for a platform name — same name always yields the
/// same color, different names are spread across the hue wheel.
pub fn badge_color(name: &str) -> Color32 {
    let hue = (fnv1a(name) % 360) as f32;
    let (r, g, b) = hsv_to_rgb(hue, 0.55, 0.80);
    Color32::from_rgb(r, g, b)
}

/// Splits a word on lowercase→uppercase transitions ("PlayStation" →
/// ["Play", "Station"]) so composite names without spaces still contribute
/// more than one initial.
fn split_camel_case(word: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    for ch in word.chars() {
        if ch.is_uppercase() && current.chars().last().is_some_and(|c| c.is_lowercase()) {
            parts.push(std::mem::take(&mut current));
        }
        current.push(ch);
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

const SKIP_WORDS: [&str; 4] = ["of", "the", "and", "&"];

/// 2-4 uppercase letters summarizing a platform name, derived from the most
/// specific segment of it (the part after the last " - ", the No-Intro/
/// Redump convention for "Manufacturer - System") — e.g.
/// "Nintendo - Super Nintendo Entertainment System" → "SNES",
/// "Sony - PlayStation" → "PS".
pub fn badge_initials(name: &str) -> String {
    let segment = name.rsplit(" - ").next().unwrap_or(name);
    let words: Vec<String> = segment
        .split_whitespace()
        .flat_map(split_camel_case)
        .filter(|w| !SKIP_WORDS.contains(&w.to_lowercase().as_str()))
        .collect();

    if words.len() >= 2 {
        words
            .iter()
            .take(4)
            .filter_map(|w| w.chars().next())
            .collect::<String>()
            .to_uppercase()
    } else if let Some(word) = words.first() {
        word.chars().take(3).collect::<String>().to_uppercase()
    } else {
        name.chars()
            .filter(|c| c.is_alphanumeric())
            .take(3)
            .collect::<String>()
            .to_uppercase()
    }
}

/// Draws a `size`x`size` circular badge for `name` at the current cursor
/// position. Allocates exactly `size`x`size` via the painter (the same
/// approach `shader_preview::draw` uses) rather than a `Frame` +
/// `centered_and_justified`, which claims all *available* space in its
/// parent `Ui` — harmless in a plain vertical layout, but inside an
/// `egui::Grid` cell that available space can be the entire row width,
/// stretching the badge into a full-width bar instead of a small circle.
pub fn draw(ui: &mut Ui, name: &str, size: f32) {
    let (response, painter) = ui.allocate_painter(egui::vec2(size, size), egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, Rounding::same(size / 2.0), badge_color(name));
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        badge_initials(name),
        egui::FontId::proportional(size * 0.32),
        Color32::WHITE,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_name_always_yields_the_same_color() {
        assert_eq!(
            badge_color("Sony - PlayStation"),
            badge_color("Sony - PlayStation")
        );
    }

    #[test]
    fn different_names_usually_yield_different_colors() {
        assert_ne!(
            badge_color("Sony - PlayStation"),
            badge_color("Nintendo - Game Boy")
        );
    }

    #[test]
    fn extracts_initials_from_a_multi_word_no_intro_style_name() {
        assert_eq!(
            badge_initials("Nintendo - Super Nintendo Entertainment System"),
            "SNES"
        );
        assert_eq!(badge_initials("Nintendo - Game Boy Advance"), "GBA");
        assert_eq!(badge_initials("Nintendo - Game Boy Color"), "GBC");
    }

    #[test]
    fn splits_camel_case_single_word_names() {
        assert_eq!(badge_initials("Sony - PlayStation"), "PS");
    }

    #[test]
    fn falls_back_to_leading_letters_for_a_short_single_word_name() {
        assert_eq!(badge_initials("Genesis"), "GEN");
    }

    #[test]
    fn never_panics_on_an_empty_or_symbol_only_name() {
        assert_eq!(badge_initials(""), "");
        let result = badge_initials("---");
        assert!(result.len() <= 3);
    }
}

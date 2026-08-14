use crate::model::Game;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulePriority {
    pub region_order: Vec<String>,
    pub language_order: Vec<String>,
    pub prefer_parent: bool,
    pub exclude_beta: bool,
    pub exclude_proto: bool,
    pub exclude_demo: bool,
    pub exclude_kiosk: bool,
    pub exclude_promo: bool,
    pub exclude_unlicensed: bool,
    pub exclude_pirate: bool,
    pub exclude_bad_dump: bool,
    /// Tie-breaker only, disabled by default so existing behavior is
    /// unchanged: among otherwise-equally-ranked candidates, prefer the one
    /// whose ROM MD5 (from the DAT) is a known RetroAchievements-compatible
    /// hash. Set together with `retroachievements_compatible_roms` by
    /// `retrotools-plugin-retroachievements` — this crate has no HTTP
    /// client of its own, it only consumes the set that plugin built.
    #[serde(default)]
    pub prefer_retroachievements_compatible: bool,
    /// Lowercased MD5 hashes known to be RetroAchievements-compatible.
    /// Deliberately not persisted with a saved rule profile (it's a large,
    /// frequently-refreshed cache, not a preference) — `#[serde(skip)]`
    /// means a profile loaded from disk always starts with this empty,
    /// independent of `prefer_retroachievements_compatible`'s saved value.
    #[serde(skip, default)]
    pub retroachievements_compatible_roms: std::collections::HashSet<String>,
}

impl Default for RulePriority {
    fn default() -> Self {
        Self {
            region_order: vec![
                "Europe".into(),
                "USA".into(),
                "Japan".into(),
                "World".into(),
            ],
            language_order: vec!["En".into()],
            prefer_parent: true,
            exclude_beta: true,
            exclude_proto: true,
            exclude_demo: true,
            exclude_kiosk: true,
            exclude_promo: true,
            exclude_unlicensed: true,
            exclude_pirate: true,
            exclude_bad_dump: true,
            prefer_retroachievements_compatible: false,
            retroachievements_compatible_roms: std::collections::HashSet::new(),
        }
    }
}

impl RulePriority {
    /// A profile that filters nothing out and keeps every alternate version,
    /// useful as a starting point for a "complete, unfiltered" collection.
    pub fn complete_no_filter() -> Self {
        Self {
            region_order: vec![],
            language_order: vec![],
            prefer_parent: true,
            exclude_beta: false,
            exclude_proto: false,
            exclude_demo: false,
            exclude_kiosk: false,
            exclude_promo: false,
            exclude_unlicensed: false,
            exclude_pirate: false,
            exclude_bad_dump: false,
            prefer_retroachievements_compatible: false,
            retroachievements_compatible_roms: std::collections::HashSet::new(),
        }
    }

    pub fn standard_europe() -> Self {
        Self {
            region_order: vec![
                "Europe".into(),
                "World".into(),
                "USA".into(),
                "Japan".into(),
            ],
            language_order: vec![
                "En".into(),
                "Fr".into(),
                "De".into(),
                "Es".into(),
                "It".into(),
            ],
            ..Self::default()
        }
    }

    pub fn standard_usa() -> Self {
        Self {
            region_order: vec![
                "USA".into(),
                "World".into(),
                "Europe".into(),
                "Japan".into(),
            ],
            language_order: vec!["En".into()],
            ..Self::default()
        }
    }

    pub fn is_excluded(&self, game: &Game) -> bool {
        (self.exclude_beta && game.is_beta)
            || (self.exclude_proto && game.is_proto)
            || (self.exclude_demo && (game.is_demo || game.is_sample))
            || (self.exclude_kiosk && game.is_kiosk)
            || (self.exclude_promo && game.is_promo)
            || (self.exclude_unlicensed && game.is_unlicensed)
            || (self.exclude_pirate && game.is_pirate)
            || (self.exclude_bad_dump && game.is_bad_dump)
    }
}

/// Rank of the best-matching entry in `order` found among `values` (case
/// insensitive). Higher is better; unmatched values rank below every match.
fn best_rank(order: &[String], values: impl Iterator<Item = String>) -> i32 {
    values
        .filter_map(|v| order.iter().position(|o| o.eq_ignore_ascii_case(&v)))
        .map(|idx| (order.len() - idx) as i32)
        .max()
        .unwrap_or(-1)
}

/// Extracts a numeric-ish score from a revision tag ("Rev 1" -> 1.0, "v1.2"
/// -> 1.2, "Rev B" -> 2.0), scaled by 100 to keep comparisons in integers.
fn revision_score(revision: &Option<String>) -> i64 {
    let Some(text) = revision else { return 0 };
    let lower = text.to_lowercase();
    let digits: String = lower
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if let Ok(value) = digits.parse::<f64>() {
        return (value * 100.0).round() as i64;
    }
    if let Some(letter) = lower.chars().rev().find(|c| c.is_ascii_alphabetic()) {
        return ((letter as i64) - ('a' as i64) + 1) * 100;
    }
    0
}

type ScoreKey = (i32, i32, i64, i32, i32, i32);

/// True when `prefer_retroachievements_compatible` is on and at least one of
/// `game`'s DAT-declared ROM MD5s is in the known-compatible set. This is a
/// proxy, not RetroAchievements' own per-console hashing algorithm — the
/// DAT's plain file MD5 happens to match RA's hash for many systems but not
/// all (some compute over a trimmed/transformed region of the ROM); good
/// enough for a tie-breaker, not represented as more precise than it is.
fn is_ra_compatible(game: &Game, rules: &RulePriority) -> bool {
    rules.prefer_retroachievements_compatible
        && !rules.retroachievements_compatible_roms.is_empty()
        && game.roms.iter().any(|rom| {
            rom.md5
                .as_deref()
                .map(|md5| {
                    rules
                        .retroachievements_compatible_roms
                        .contains(&md5.to_lowercase())
                })
                .unwrap_or(false)
        })
}

fn compute_score(game: &Game, rules: &RulePriority) -> ScoreKey {
    let region_rank = best_rank(
        &rules.region_order,
        game.regions.iter().map(|r| r.0.clone()),
    );
    let language_rank = best_rank(
        &rules.language_order,
        game.languages.iter().map(|l| l.0.clone()),
    );
    let revision = revision_score(&game.revision);
    let parent_bonus = if rules.prefer_parent && game.clone_of.is_none() {
        1
    } else {
        0
    };
    let alt_penalty = if game.is_alt { -1 } else { 0 };
    let ra_bonus = if is_ra_compatible(game, rules) { 1 } else { 0 };
    (
        region_rank,
        language_rank,
        revision,
        parent_bonus,
        alt_penalty,
        ra_bonus,
    )
}

/// A "release": one or more disc/file entries that together form a single
/// purchasable version of a game (e.g. "Game (Europe) (Disc 1)" + "(Disc 2)").
#[derive(Debug, Clone)]
struct ReleaseCandidate<'a> {
    base_name: String,
    discs: Vec<&'a Game>,
}

impl<'a> ReleaseCandidate<'a> {
    fn representative(&self) -> &'a Game {
        self.discs[0]
    }
}

/// Strips a `(Disc N)` token (case-insensitive) from a ROM name so that every
/// disc of the same release shares an identical grouping key, and returns the
/// disc number if one was found.
fn strip_disc_token(name: &str) -> (String, Option<u32>) {
    let lower = name.to_lowercase();
    if let Some(start) = lower.find("(disc ") {
        if let Some(rel_end) = lower[start..].find(')') {
            let end = start + rel_end + 1;
            let inner = &name[start + 6..end - 1];
            let disc_number = inner.trim().parse::<u32>().ok();
            let mut stripped = String::with_capacity(name.len());
            stripped.push_str(name[..start].trim_end());
            stripped.push_str(&name[end..]);
            return (stripped.trim().to_string(), disc_number);
        }
    }
    (name.to_string(), None)
}

/// Reduces a ROM name to its bare title by stripping every parenthetical
/// group (region, language, version/disc tags, ...). Used as the family
/// grouping key: unlike `cloneof` — which many real-world DAT files leave
/// unset or inconsistent for discs beyond the first — this is robust
/// regardless of how well-formed the DAT's clone metadata is.
fn canonical_title(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    let mut depth = 0i32;
    for ch in name.chars() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            _ if depth == 0 => result.push(ch),
            _ => {}
        }
    }
    result.trim().to_string()
}

fn build_families(games: &[Game]) -> BTreeMap<String, Vec<ReleaseCandidate<'_>>> {
    let mut releases: BTreeMap<(String, String), Vec<&Game>> = BTreeMap::new();
    for game in games {
        let (base_name, _disc) = strip_disc_token(&game.name);
        releases
            .entry((canonical_title(&game.name), base_name))
            .or_default()
            .push(game);
    }

    let mut families: BTreeMap<String, Vec<ReleaseCandidate<'_>>> = BTreeMap::new();
    for ((family, base_name), mut discs) in releases {
        discs.sort_by_key(|g| strip_disc_token(&g.name).1.unwrap_or(0));
        families
            .entry(family)
            .or_default()
            .push(ReleaseCandidate { base_name, discs });
    }
    families
}

#[derive(Debug, Clone)]
pub struct SelectionExplanation {
    pub family: String,
    pub chosen: String,
    pub runner_up: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct SelectionResult {
    pub kept: Vec<Game>,
    pub removed: Vec<Game>,
    pub explanations: Vec<SelectionExplanation>,
}

fn explain(winner_score: ScoreKey, runner_up: Option<(&ReleaseCandidate, ScoreKey)>) -> String {
    let Some((runner, runner_score)) = runner_up else {
        return "only remaining candidate for this game".to_string();
    };
    if winner_score.0 != runner_score.0 {
        format!(
            "region priority {} beats {} (from '{}')",
            winner_score.0, runner_score.0, runner.base_name
        )
    } else if winner_score.1 != runner_score.1 {
        format!(
            "language priority {} beats {} (from '{}')",
            winner_score.1, runner_score.1, runner.base_name
        )
    } else if winner_score.2 != runner_score.2 {
        format!("higher revision score beats '{}'", runner.base_name)
    } else if winner_score.3 != runner_score.3 {
        "parent release preferred over clone".to_string()
    } else {
        format!("tie-broken alphabetically over '{}'", runner.base_name)
    }
}

/// Selects one release per game family according to `rules`, keeping every
/// disc/file that belongs to the winning release (multi-disc sets are never
/// split across regions), and reports why each choice was made.
pub fn preview_selection(games: &[Game], rules: &RulePriority) -> SelectionResult {
    let mut result = SelectionResult::default();

    let mut eligible = Vec::new();
    for game in games {
        if rules.is_excluded(game) {
            result.removed.push(game.clone());
        } else {
            eligible.push(game.clone());
        }
    }

    let families = build_families(&eligible);

    for (family, mut candidates) in families {
        candidates.sort_by(|a, b| {
            let score_a = compute_score(a.representative(), rules);
            let score_b = compute_score(b.representative(), rules);
            score_b
                .cmp(&score_a)
                .then_with(|| a.base_name.cmp(&b.base_name))
        });

        let Some(winner) = candidates.first() else {
            continue;
        };
        let winner_score = compute_score(winner.representative(), rules);
        let runner_up = candidates
            .get(1)
            .map(|c| (c, compute_score(c.representative(), rules)));

        result.explanations.push(SelectionExplanation {
            family: family.clone(),
            chosen: winner.base_name.clone(),
            runner_up: runner_up.map(|(c, _)| c.base_name.clone()),
            reason: explain(winner_score, runner_up),
        });

        for disc in &winner.discs {
            result.kept.push((*disc).clone());
        }
        for candidate in &candidates[1..] {
            for disc in &candidate.discs {
                result.removed.push((*disc).clone());
            }
        }
    }

    result
}

pub fn select_one_game_one_rom(games: &[Game], rules: &RulePriority) -> Vec<Game> {
    preview_selection(games, rules).kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dat::parse_dat_str;

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<datafile>
  <header><name>Test</name></header>
  <game name="Game (Europe)">
    <rom name="Game (Europe).bin" size="1" crc="00000001"/>
  </game>
  <game name="Game (USA)" cloneof="Game (Europe)">
    <rom name="Game (USA).bin" size="1" crc="00000002"/>
  </game>
  <game name="Game (Japan)" cloneof="Game (Europe)">
    <rom name="Game (Japan).bin" size="1" crc="00000003"/>
  </game>
  <game name="Game (Europe) (Beta)" cloneof="Game (Europe)">
    <rom name="Game (Europe) (Beta).bin" size="1" crc="00000004"/>
  </game>
</datafile>"#;

    const MULTI_DISC: &str = r#"<?xml version="1.0"?>
<datafile>
  <header><name>Test</name></header>
  <game name="RPG (Europe) (Disc 1)">
    <rom name="RPG (Europe) (Disc 1).bin" size="1" crc="00000001"/>
  </game>
  <game name="RPG (Europe) (Disc 2)">
    <rom name="RPG (Europe) (Disc 2).bin" size="1" crc="00000002"/>
  </game>
  <game name="RPG (USA) (Disc 1)" cloneof="RPG (Europe) (Disc 1)">
    <rom name="RPG (USA) (Disc 1).bin" size="1" crc="00000003"/>
  </game>
  <game name="RPG (USA) (Disc 2)" cloneof="RPG (Europe) (Disc 1)">
    <rom name="RPG (USA) (Disc 2).bin" size="1" crc="00000004"/>
  </game>
</datafile>"#;

    #[test]
    fn picks_highest_priority_region_and_excludes_beta() {
        let gameset = parse_dat_str(SAMPLE, "Test").unwrap();
        let rules = RulePriority::default();
        let kept = select_one_game_one_rom(&gameset.games, &rules);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].name, "Game (Europe)");
    }

    #[test]
    fn keeps_all_discs_of_the_winning_release() {
        let gameset = parse_dat_str(MULTI_DISC, "Test").unwrap();
        let rules = RulePriority::default();
        let kept = select_one_game_one_rom(&gameset.games, &rules);
        assert_eq!(kept.len(), 2);
        assert!(kept.iter().all(|g| g.name.starts_with("RPG (Europe)")));
    }

    // Two entries that end up perfectly tied on every existing criterion
    // (same region, same canonical title after tag-stripping, neither a
    // clone, neither carrying a recognized tag like beta/alt/rev) so that
    // RA compatibility is the only thing that can break the tie.
    const TIED_ALTS: &str = r#"<?xml version="1.0"?>
<datafile>
  <header><name>Test</name></header>
  <game name="Game (Europe) (Version A)">
    <rom name="Game (Europe) (Version A).bin" size="1" crc="00000001" md5="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"/>
  </game>
  <game name="Game (Europe) (Version B)">
    <rom name="Game (Europe) (Version B).bin" size="1" crc="00000002" md5="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"/>
  </game>
</datafile>"#;

    #[test]
    fn ra_compatibility_only_breaks_ties_when_the_flag_is_enabled() {
        let gameset = parse_dat_str(TIED_ALTS, "Test").unwrap();

        // Disabled (default): with every other criterion tied, selection
        // falls back to its existing deterministic order — unaffected by
        // the RA fields existing at all.
        let rules = RulePriority::default();
        let kept_default = select_one_game_one_rom(&gameset.games, &rules);
        assert_eq!(kept_default.len(), 1);

        // Enabled, with only "Version B"'s MD5 known RA-compatible: it now
        // wins the tie, purely because of the new tie-breaker.
        let mut compatible_roms = std::collections::HashSet::new();
        compatible_roms.insert("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string());
        let rules = RulePriority {
            prefer_retroachievements_compatible: true,
            retroachievements_compatible_roms: compatible_roms,
            ..RulePriority::default()
        };
        let kept = select_one_game_one_rom(&gameset.games, &rules);
        assert_eq!(kept[0].name, "Game (Europe) (Version B)");
    }

    #[test]
    fn complete_no_filter_keeps_beta_versions() {
        let gameset = parse_dat_str(SAMPLE, "Test").unwrap();
        let rules = RulePriority::complete_no_filter();
        let preview = preview_selection(&gameset.games, &rules);
        assert!(preview
            .kept
            .iter()
            .chain(preview.removed.iter())
            .any(|g| g.name.contains("Beta")));
        // With no region priorities configured, only one release still wins per family.
        assert_eq!(preview.kept.len(), 1);
    }

    #[test]
    fn explanations_reference_region_priority() {
        let gameset = parse_dat_str(SAMPLE, "Test").unwrap();
        let rules = RulePriority::default();
        let preview = preview_selection(&gameset.games, &rules);
        assert_eq!(preview.explanations.len(), 1);
        assert!(preview.explanations[0].reason.contains("region priority"));
    }
}

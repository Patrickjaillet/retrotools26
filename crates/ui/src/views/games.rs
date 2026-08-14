use crate::state::{AppState, GameScanStatus, GamesViewMode, SortColumn, StatusFilter};
use egui::{Color32, RichText, ScrollArea, Ui};
use retrotools_core::Game;

fn status_label(status: GameScanStatus) -> (&'static str, Color32) {
    match status {
        GameScanStatus::Matched => ("Matched", Color32::from_rgb(76, 175, 80)),
        GameScanStatus::Corrupt => ("Corrupt", Color32::from_rgb(244, 67, 54)),
        GameScanStatus::Missing => ("Missing", Color32::from_rgb(255, 152, 0)),
        GameScanStatus::NotScanned => ("Not scanned", Color32::GRAY),
    }
}

fn matches_filter(state: &AppState, game: &Game) -> bool {
    let filter = &state.games_filter;

    if !filter.search.is_empty()
        && !game
            .name
            .to_lowercase()
            .contains(&filter.search.to_lowercase())
    {
        return false;
    }
    if let Some(region) = &filter.region {
        if !game.regions.iter().any(|r| &r.0 == region) {
            return false;
        }
    }
    if let Some(language) = &filter.language {
        if !game.languages.iter().any(|l| &l.0 == language) {
            return false;
        }
    }
    let status = state.game_scan_status(&game.name);
    match filter.status {
        StatusFilter::All => true,
        StatusFilter::Matched => status == GameScanStatus::Matched,
        StatusFilter::Corrupt => status == GameScanStatus::Corrupt,
        StatusFilter::Unknown => status == GameScanStatus::NotScanned,
        StatusFilter::Missing => status == GameScanStatus::Missing,
    }
}

fn game_size(game: &Game) -> u64 {
    game.roms.iter().map(|r| r.size).sum()
}

fn sort_key_order(state: &AppState, status: GameScanStatus) -> u8 {
    let _ = state;
    match status {
        GameScanStatus::Matched => 0,
        GameScanStatus::Corrupt => 1,
        GameScanStatus::Missing => 2,
        GameScanStatus::NotScanned => 3,
    }
}

pub fn show(ui: &mut Ui, state: &mut AppState) {
    let Some(gameset) = state.current_gameset().cloned() else {
        ui.heading("Games");
        ui.add_space(12.0);
        ui.label(RichText::new("Select a platform in the Platforms tab first.").weak());
        return;
    };

    ui.horizontal(|ui| {
        crate::platform_badge::draw(ui, &gameset.platform, 26.0);
        ui.heading(format!("Games — {}", gameset.platform));
    });
    ui.add_space(8.0);

    let all_regions: std::collections::BTreeSet<String> = gameset
        .games
        .iter()
        .flat_map(|g| g.regions.iter().map(|r| r.0.clone()))
        .collect();
    let all_languages: std::collections::BTreeSet<String> = gameset
        .games
        .iter()
        .flat_map(|g| g.languages.iter().map(|l| l.0.clone()))
        .collect();

    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut state.games_filter.search);

        ui.separator();
        egui::ComboBox::from_id_source("region_filter")
            .selected_text(
                state
                    .games_filter
                    .region
                    .clone()
                    .unwrap_or_else(|| "Any region".into()),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.games_filter.region, None, "Any region");
                for region in &all_regions {
                    ui.selectable_value(
                        &mut state.games_filter.region,
                        Some(region.clone()),
                        region,
                    );
                }
            });

        egui::ComboBox::from_id_source("language_filter")
            .selected_text(
                state
                    .games_filter
                    .language
                    .clone()
                    .unwrap_or_else(|| "Any language".into()),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.games_filter.language, None, "Any language");
                for language in &all_languages {
                    ui.selectable_value(
                        &mut state.games_filter.language,
                        Some(language.clone()),
                        language,
                    );
                }
            });

        egui::ComboBox::from_id_source("status_filter")
            .selected_text(format!("{:?}", state.games_filter.status))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.games_filter.status, StatusFilter::All, "All");
                ui.selectable_value(
                    &mut state.games_filter.status,
                    StatusFilter::Matched,
                    "Matched",
                );
                ui.selectable_value(
                    &mut state.games_filter.status,
                    StatusFilter::Corrupt,
                    "Corrupt",
                );
                ui.selectable_value(
                    &mut state.games_filter.status,
                    StatusFilter::Unknown,
                    "Not scanned",
                );
                ui.selectable_value(
                    &mut state.games_filter.status,
                    StatusFilter::Missing,
                    "Missing",
                );
            });
    });

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Sort by:").weak());
        for (label, column) in [
            ("Name", SortColumn::Name),
            ("Region", SortColumn::Region),
            ("Status", SortColumn::Status),
            ("Size", SortColumn::Size),
        ] {
            let selected = state.games_filter.sort == column;
            let text = if selected {
                format!(
                    "{label} {}",
                    if state.games_filter.sort_ascending {
                        "^"
                    } else {
                        "v"
                    }
                )
            } else {
                label.to_string()
            };
            if ui.selectable_label(selected, text).clicked() {
                if selected {
                    state.games_filter.sort_ascending = !state.games_filter.sort_ascending;
                } else {
                    state.games_filter.sort = column;
                    state.games_filter.sort_ascending = true;
                }
            }
        }

        ui.separator();
        ui.label(RichText::new("View:").weak());
        if ui
            .selectable_label(state.games_filter.view_mode == GamesViewMode::List, "List")
            .clicked()
        {
            state.games_filter.view_mode = GamesViewMode::List;
        }
        if ui
            .selectable_label(state.games_filter.view_mode == GamesViewMode::Grid, "Grid")
            .clicked()
        {
            state.games_filter.view_mode = GamesViewMode::Grid;
        }
    });

    ui.add_space(8.0);
    ui.separator();

    let mut filtered: Vec<&Game> = gameset
        .games
        .iter()
        .filter(|g| matches_filter(state, g))
        .collect();
    match state.games_filter.sort {
        SortColumn::Name => filtered.sort_by(|a, b| a.name.cmp(&b.name)),
        SortColumn::Region => filtered.sort_by(|a, b| {
            let ra = a.regions.first().map(|r| r.0.as_str()).unwrap_or("");
            let rb = b.regions.first().map(|r| r.0.as_str()).unwrap_or("");
            ra.cmp(rb).then_with(|| a.name.cmp(&b.name))
        }),
        SortColumn::Status => filtered.sort_by(|a, b| {
            sort_key_order(state, state.game_scan_status(&a.name))
                .cmp(&sort_key_order(state, state.game_scan_status(&b.name)))
                .then_with(|| a.name.cmp(&b.name))
        }),
        SortColumn::Size => filtered.sort_by_key(|a| game_size(a)),
    }
    if !state.games_filter.sort_ascending {
        filtered.reverse();
    }

    ui.label(RichText::new(format!("{} game(s)", filtered.len())).weak());
    ui.add_space(6.0);

    ui.columns(2, |columns| {
        ScrollArea::vertical()
            .id_source("games_list")
            .show(&mut columns[0], |ui| match state.games_filter.view_mode {
                GamesViewMode::List => show_list(ui, state, &filtered),
                GamesViewMode::Grid => show_grid(ui, state, &filtered),
            });

        ScrollArea::vertical()
            .id_source("game_details")
            .show(&mut columns[1], |ui| {
                show_details(ui, state, &gameset, &filtered);
            });
    });
}

fn show_list(ui: &mut Ui, state: &mut AppState, filtered: &[&Game]) {
    for game in filtered {
        let selected = state.selected_game.as_deref() == Some(game.name.as_str());
        let status = state.game_scan_status(&game.name);
        let (status_text, status_color) = status_label(status);
        let kept_marker = match state.is_kept(&game.name) {
            Some(true) => " *KEPT*",
            Some(false) => " (removed)",
            None => "",
        };

        ui.horizontal(|ui| {
            if ui.selectable_label(selected, &game.name).clicked() {
                state.selected_game = Some(game.name.clone());
            }
            ui.label(RichText::new(status_text).color(status_color).small());
            if !kept_marker.is_empty() {
                ui.label(RichText::new(kept_marker).small().weak());
            }
        });
    }
}

/// A card-based grid layout for the games list. There is no artwork/box-art
/// source available yet (no scraper — see Phase 7), so each card shows a
/// colored placeholder tile (by scan status) instead of real cover art; the
/// layout itself is real and usable today, and can grow a thumbnail once a
/// scraper exists without any other change.
fn show_grid(ui: &mut Ui, state: &mut AppState, filtered: &[&Game]) {
    const CARD_WIDTH: f32 = 180.0;
    // Must match `show_game_card`'s `Frame::group(..).inner_margin(..)`
    // below — the frame's *rendered* width is `CARD_WIDTH` plus this margin
    // on both sides, not `CARD_WIDTH` alone. The row-width math has to use
    // the full rendered size or it under-counts each card's footprint and
    // packs one too many per row, which then overflows into the details
    // column on the right (found via a real 34410-game list, where the
    // extra ~20px per card that a small test list didn't make visible
    // added up to a real overlap).
    const CARD_FRAME_MARGIN: f32 = 10.0;
    const CARD_RENDERED_WIDTH: f32 = CARD_WIDTH + 2.0 * CARD_FRAME_MARGIN;

    // Explicit row-chunking based on the actual measured available width,
    // rather than `horizontal_wrapped`'s automatic wrapping — inside a
    // `ScrollArea` nested in an `egui::Ui::columns` cell, `horizontal_wrapped`
    // was measuring the *whole window's* width rather than the constrained
    // half-width column, so cards kept flowing past the column boundary and
    // underneath the details panel on the right instead of wrapping.
    let item_spacing = ui.spacing().item_spacing.x;
    let card_slot_width = CARD_RENDERED_WIDTH + item_spacing;
    let available_width = ui.available_width();
    let cards_per_row =
        (((available_width + item_spacing) / card_slot_width).floor() as usize).max(1);

    for chunk in filtered.chunks(cards_per_row) {
        ui.horizontal(|ui| {
            for game in chunk {
                show_game_card(ui, state, game, CARD_WIDTH);
            }
        });
    }
}

fn show_game_card(ui: &mut Ui, state: &mut AppState, game: &Game, card_width: f32) {
    let selected = state.selected_game.as_deref() == Some(game.name.as_str());
    let status = state.game_scan_status(&game.name);
    let (status_text, status_color) = status_label(status);

    let frame = egui::Frame::group(ui.style())
        .rounding(egui::Rounding::same(8.0))
        .inner_margin(egui::Margin::same(10.0))
        .fill(if selected {
            ui.visuals().selection.bg_fill.linear_multiply(0.35)
        } else {
            ui.visuals().extreme_bg_color
        });

    let response = frame.show(ui, |ui| {
        ui.set_width(card_width);
        ui.vertical(|ui| {
            ui.add(egui::widgets::Separator::default().spacing(0.0).grow(0.0));
            // Allocates its exact size via the painter rather than `Frame` +
            // `centered_and_justified` — the latter claims all *available*
            // space in its parent `Ui`, which stretched this tile (and the
            // whole card) into a full-width bar overlapping neighboring
            // cards on large game lists (see `platform_badge::draw` for the
            // same bug, hit first).
            let (tile_rect, _) =
                ui.allocate_exact_size(egui::vec2(card_width - 20.0, 60.0), egui::Sense::hover());
            ui.painter().rect_filled(
                tile_rect,
                egui::Rounding::same(6.0),
                status_color.linear_multiply(0.5),
            );
            ui.painter().text(
                tile_rect.center(),
                egui::Align2::CENTER_CENTER,
                status_text,
                egui::FontId::proportional(14.0),
                Color32::WHITE,
            );
            ui.add_space(6.0);
            ui.label(RichText::new(&game.name).strong().small());
            if let Some(region) = game.regions.first() {
                ui.label(RichText::new(&region.0).weak().small());
            }
            if let Some(true) = state.is_kept(&game.name) {
                ui.label(
                    RichText::new("KEPT")
                        .small()
                        .color(Color32::from_rgb(76, 175, 80)),
                );
            }
        });
    });

    if response.response.interact(egui::Sense::click()).clicked() {
        state.selected_game = Some(game.name.clone());
    }
}

fn show_details(
    ui: &mut Ui,
    state: &AppState,
    gameset: &retrotools_core::GameSet,
    filtered: &[&Game],
) {
    let Some(selected_name) = &state.selected_game else {
        ui.label(RichText::new("Select a game to see its details.").weak());
        return;
    };
    let Some(game) = filtered
        .iter()
        .find(|g| &g.name == selected_name)
        .copied()
        .or_else(|| gameset.games.iter().find(|g| &g.name == selected_name))
    else {
        ui.label(RichText::new("Select a game to see its details.").weak());
        return;
    };

    ui.heading(&game.name);
    ui.add_space(8.0);

    egui::Grid::new("game_details_grid")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label(RichText::new("Status").weak());
            let (status_text, status_color) = status_label(state.game_scan_status(&game.name));
            ui.label(RichText::new(status_text).color(status_color));
            ui.end_row();

            ui.label(RichText::new("1G1R selection").weak());
            ui.label(match state.is_kept(&game.name) {
                Some(true) => "Kept",
                Some(false) => "Removed",
                None => "Not previewed",
            });
            ui.end_row();

            ui.label(RichText::new("Regions").weak());
            ui.label(if game.regions.is_empty() {
                "-".to_string()
            } else {
                game.regions
                    .iter()
                    .map(|r| r.0.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            });
            ui.end_row();

            ui.label(RichText::new("Languages").weak());
            ui.label(if game.languages.is_empty() {
                "-".to_string()
            } else {
                game.languages
                    .iter()
                    .map(|l| l.0.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            });
            ui.end_row();

            ui.label(RichText::new("Total size").weak());
            ui.label(format_size(game_size(game)));
            ui.end_row();

            ui.label(RichText::new("Tags").weak());
            ui.label(tags_summary(game));
            ui.end_row();

            if let Some(rec) = retrotools_plugin_core_advisor::find_recommendation(
                &state.core_advisor_db,
                &gameset.platform,
                &game.name,
            ) {
                ui.label(RichText::new("Recommended core").weak());
                let flag = if rec.known_problematic {
                    " ⚠ known problematic"
                } else {
                    ""
                };
                ui.label(format!(
                    "{} ({} confidence){flag}",
                    rec.core, rec.confidence
                ));
                ui.end_row();
                if !rec.note.is_empty() {
                    ui.label(RichText::new("Core note").weak());
                    ui.label(&rec.note);
                    ui.end_row();
                }
            }

            if state.config.retroachievements.is_configured() {
                let hashes = retrotools_plugin_retroachievements::load_cached_hashes_for_platform(
                    &gameset.platform,
                );
                let status = if hashes.is_empty() {
                    "Not synced yet"
                } else if game.roms.iter().any(|r| {
                    r.md5
                        .as_deref()
                        .map(|md5| hashes.contains(&md5.to_lowercase()))
                        .unwrap_or(false)
                }) {
                    "Compatible (known hash)"
                } else {
                    "No known hash"
                };
                ui.label(RichText::new("RetroAchievements").weak());
                ui.label(status);
                ui.end_row();
            }
        });

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);
    ui.label(RichText::new("ROM files").strong());
    ui.add_space(4.0);

    egui::Grid::new("rom_files_grid")
        .num_columns(3)
        .striped(true)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label(RichText::new("Name").weak());
            ui.label(RichText::new("Size").weak());
            ui.label(RichText::new("CRC32").weak());
            ui.end_row();

            for rom in &game.roms {
                ui.label(&rom.name);
                ui.label(format_size(rom.size));
                ui.label(rom.crc32.as_deref().unwrap_or("-"));
                ui.end_row();
            }
        });
}

fn tags_summary(game: &Game) -> String {
    let mut tags = Vec::new();
    if game.is_beta {
        tags.push("Beta");
    }
    if game.is_proto {
        tags.push("Proto");
    }
    if game.is_demo {
        tags.push("Demo");
    }
    if game.is_sample {
        tags.push("Sample");
    }
    if game.is_kiosk {
        tags.push("Kiosk");
    }
    if game.is_promo {
        tags.push("Promo");
    }
    if game.is_unlicensed {
        tags.push("Unlicensed");
    }
    if game.is_pirate {
        tags.push("Pirate");
    }
    if game.is_bad_dump {
        tags.push("Bad Dump");
    }
    if game.is_alt {
        tags.push("Alt");
    }
    if tags.is_empty() {
        "-".to_string()
    } else {
        tags.join(", ")
    }
}

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.2} {}", UNITS[unit])
    }
}

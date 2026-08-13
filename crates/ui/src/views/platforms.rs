use crate::state::AppState;
use egui::{RichText, Ui};

pub fn show(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.heading("Platforms");
        ui.add_space(12.0);
        if ui.button("Import DAT...").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("DAT files", &["dat", "xml", "zip"])
                .pick_file()
            {
                state.import_dat(path);
            }
        }
    });
    ui.add_space(8.0);
    ui.label(
        RichText::new("Drag & drop a DAT file (.dat/.xml/.zip) anywhere in the window to import it.")
            .weak()
            .small(),
    );
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    missing_dats_section(ui, state);
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    if state.library.entries().is_empty() {
        ui.label(RichText::new("No DAT imported yet.").weak());
        return;
    }

    let platforms: Vec<(String, String, String, usize)> = state
        .library
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.gameset.platform.clone(),
                entry.gameset.dat_type.to_string(),
                entry.gameset.dat_version.clone(),
                entry.gameset.games.len(),
            )
        })
        .collect();

    egui::Grid::new("platforms_grid")
        .num_columns(5)
        .striped(true)
        .spacing([16.0, 6.0])
        .show(ui, |ui| {
            ui.label(RichText::new("Platform").strong());
            ui.label(RichText::new("DAT type").strong());
            ui.label(RichText::new("Version").strong());
            ui.label(RichText::new("Games").strong());
            ui.label("");
            ui.end_row();

            for (platform, dat_type, version, count) in platforms {
                let selected = state.selected_platform.as_deref() == Some(platform.as_str());
                if ui.selectable_label(selected, &platform).clicked() {
                    state.selected_platform = Some(platform.clone());
                    state.match_report = None;
                    state.selection_preview = None;
                    state.scan_outcome = None;
                }
                ui.label(dat_type);
                ui.label(version);
                ui.label(count.to_string());
                if ui.small_button("Remove").clicked() {
                    state.library.remove(&platform);
                    if state.selected_platform.as_deref() == Some(platform.as_str()) {
                        state.selected_platform = None;
                    }
                }
                ui.end_row();
            }
        });
}

fn missing_dats_section(ui: &mut Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Missing DATs").strong());
        ui.add_space(8.0);
        let can_detect = !state.config.rom_directories.is_empty();
        if ui
            .add_enabled(can_detect, egui::Button::new("Detect missing DATs..."))
            .on_hover_text("Scans the ROM directories configured in Settings for platform folders with no matching DAT")
            .clicked()
        {
            state.detect_missing_dats();
        }
        if !can_detect {
            ui.label(RichText::new("Add a ROM directory in Settings first.").weak().small());
        }
    });

    if state.missing_dat_platforms.is_empty() {
        return;
    }

    ui.add_space(6.0);
    let names = state.missing_dat_platforms.clone();
    for name in names {
        ui.horizontal(|ui| {
            ui.label(&name);
            let has_source = state
                .config
                .dat_sources
                .iter()
                .any(|s| s.name.eq_ignore_ascii_case(&name));
            let downloading = state.dat_sources_updating.contains(&name);
            if has_source {
                if ui
                    .add_enabled(!downloading, egui::Button::new("Download & import"))
                    .clicked()
                {
                    state.start_missing_dat_download(&name);
                }
                if downloading {
                    ui.spinner();
                }
            } else {
                ui.label(
                    RichText::new("No tracked source with a matching name — add one in Settings.")
                        .weak()
                        .small(),
                );
            }
        });
    }
}

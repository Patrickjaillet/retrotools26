use crate::state::AppState;
use egui::{RichText, Ui};

pub fn show(ui: &mut Ui, state: &mut AppState) {
    ui.heading("Plugins");
    ui.add_space(8.0);
    ui.label(
        RichText::new(
            "Optional modules that extend Retro Tools 2026 without touching the core engine. \
             See docs/PLUGIN_DEV.md to write your own.",
        )
        .weak()
        .small(),
    );
    ui.add_space(12.0);

    ui.horizontal(|ui| {
        if ui.button("Choose source folder...").clicked() {
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                state.plugin_source_dir = Some(folder);
            }
        }
        ui.label(
            state
                .plugin_source_dir
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "No source folder (only needed by some plugins)".to_string()),
        );
    });

    ui.horizontal(|ui| {
        if ui.button("Choose output folder...").clicked() {
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                state.plugin_output_dir = Some(folder);
            }
        }
        ui.label(
            state
                .plugin_output_dir
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "No output folder selected".to_string()),
        );
    });

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(12.0);

    let plugin_ids: Vec<(String, String, String)> = state
        .plugin_registry
        .plugins()
        .iter()
        .map(|p| (p.id().to_string(), p.name().to_string(), p.description().to_string()))
        .collect();

    for (id, name, description) in plugin_ids {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(&name).strong());
                    ui.label(RichText::new(&description).weak().small());
                });
                if ui.button("Run").clicked() {
                    state.run_plugin(&id);
                }
            });
            if let Some(outcome) = state.plugin_last_outcomes.get(&id) {
                match outcome {
                    Ok(summary) => {
                        ui.label(RichText::new(format!("Last run: {summary}")).color(egui::Color32::from_rgb(76, 175, 80)));
                    }
                    Err(err) => {
                        ui.label(RichText::new(format!("Last run failed: {err}")).color(egui::Color32::from_rgb(244, 67, 54)));
                    }
                }
            }
        });
        ui.add_space(8.0);
    }
}

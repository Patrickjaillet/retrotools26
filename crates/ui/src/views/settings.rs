use crate::i18n::{Key, Language};
use crate::state::{AppState, ToastKind};
use egui::{RichText, Ui};
use retrotools_common::config::ThemePreference;

pub fn show(ui: &mut Ui, state: &mut AppState) {
    ui.heading(state.t(Key::SettingsTitle));
    ui.add_space(12.0);

    ui.label(RichText::new(state.t(Key::SettingsAppearance)).strong());
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(state.t(Key::SettingsTheme));
        egui::ComboBox::from_id_source("theme_combo")
            .selected_text(format!("{:?}", state.config.theme))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.config.theme, ThemePreference::Light, "Light");
                ui.selectable_value(&mut state.config.theme, ThemePreference::Dark, "Dark");
                ui.selectable_value(&mut state.config.theme, ThemePreference::System, "System");
            });
    });

    ui.horizontal(|ui| {
        ui.label("Accent color:");
        let mut color = [
            state.config.accent_color[0],
            state.config.accent_color[1],
            state.config.accent_color[2],
        ];
        if ui.color_edit_button_srgb(&mut color).changed() {
            state.config.accent_color = color;
        }
    });

    ui.horizontal(|ui| {
        ui.label(state.t(Key::SettingsLanguage));
        let current = state.language();
        egui::ComboBox::from_id_source("language_combo")
            .selected_text(current.label())
            .show_ui(ui, |ui| {
                for lang in Language::all() {
                    if ui.selectable_label(current == *lang, lang.label()).clicked() {
                        state.config.language = lang.code().to_string();
                    }
                }
            });
    });

    ui.horizontal(|ui| {
        ui.label("UI scale:");
        ui.add(egui::Slider::new(&mut state.config.ui_scale, 0.75..=2.0).step_by(0.05));
    });

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(12.0);

    ui.label(RichText::new("ROM directories").strong());
    ui.add_space(6.0);
    let mut remove_rom = None;
    for (index, dir) in state.config.rom_directories.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(dir.display().to_string());
            if ui.small_button("Remove").clicked() {
                remove_rom = Some(index);
            }
        });
    }
    if let Some(index) = remove_rom {
        state.config.rom_directories.remove(index);
    }
    if ui.button("Add ROM directory...").clicked() {
        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
            state.config.rom_directories.push(dir);
        }
    }

    ui.add_space(16.0);
    ui.label(RichText::new("DAT directories").strong());
    ui.add_space(6.0);
    let mut remove_dat = None;
    for (index, dir) in state.config.dat_directories.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(dir.display().to_string());
            if ui.small_button("Remove").clicked() {
                remove_dat = Some(index);
            }
        });
    }
    if let Some(index) = remove_dat {
        state.config.dat_directories.remove(index);
    }
    if ui.button("Add DAT directory...").clicked() {
        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
            state.config.dat_directories.push(dir);
        }
    }

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(12.0);

    ui.label(RichText::new("DAT update sources").strong());
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Track a direct DAT/ZIP download URL under a name; Update re-downloads it and \
             reports whether the version changed. No-Intro/Redump don't publish an unauthenticated \
             discovery API, so the URL has to be one you copy yourself.",
        )
        .weak()
        .small(),
    );
    ui.add_space(6.0);

    let mut remove_source = None;
    let mut update_source = None;
    let source_names: Vec<String> = state.config.dat_sources.iter().map(|s| s.name.clone()).collect();
    for name in &source_names {
        let url = state
            .config
            .dat_sources
            .iter()
            .find(|s| &s.name == name)
            .map(|s| s.url.clone())
            .unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label(RichText::new(name).strong());
            ui.label(RichText::new(&url).weak().small());
            let updating = state.dat_sources_updating.contains(name);
            if ui.add_enabled(!updating, egui::Button::new("Update")).clicked() {
                update_source = Some(name.clone());
            }
            if updating {
                ui.spinner();
            }
            if ui.small_button("Remove").clicked() {
                remove_source = Some(name.clone());
            }
        });
        if let Some(result) = state.dat_source_last_results.get(name) {
            match result {
                Ok(summary) => {
                    ui.label(RichText::new(summary).color(egui::Color32::from_rgb(76, 175, 80)).small());
                }
                Err(err) => {
                    ui.label(RichText::new(err).color(egui::Color32::from_rgb(244, 67, 54)).small());
                }
            }
        }
    }
    if let Some(name) = remove_source {
        state.remove_dat_source(&name);
    }
    if let Some(name) = update_source {
        state.start_dat_source_update(&name);
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut state.new_dat_source_name);
        ui.label("URL:");
        ui.text_edit_singleline(&mut state.new_dat_source_url);
        if ui.button("Add source").clicked() {
            state.add_dat_source();
        }
    });

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(12.0);

    ui.label(RichText::new("Advanced").strong());
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Log level:");
        egui::ComboBox::from_id_source("log_level_combo")
            .selected_text(state.config.log_level.clone())
            .show_ui(ui, |ui| {
                for level in ["trace", "debug", "info", "warn", "error"] {
                    ui.selectable_value(&mut state.config.log_level, level.to_string(), level);
                }
            });
    });
    ui.checkbox(&mut state.config.check_updates_on_startup, "Check for updates on startup");

    ui.add_space(20.0);
    if ui.button(state.t(Key::SaveSettingsButton)).clicked() {
        match state.config.save() {
            Ok(()) => state.toast(ToastKind::Success, "Settings saved"),
            Err(err) => state.toast(ToastKind::Error, format!("Cannot save settings: {err}")),
        }
    }
}

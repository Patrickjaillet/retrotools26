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

    ui.label(RichText::new("ScreenScraper.fr (metadata scraper)").strong());
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Your own ScreenScraper.fr developer and account credentials — never bundled with this \
             app, stored encrypted on this machine (Windows DPAPI, tied to your Windows login). \
             The scraper plugin stays inactive until these are filled in.",
        )
        .weak()
        .small(),
    );
    ui.add_space(6.0);
    if state.config.screenscraper.is_configured() {
        ui.horizontal(|ui| {
            ui.label(RichText::new("✓ Credentials configured").color(egui::Color32::from_rgb(76, 175, 80)));
            if ui.small_button("Clear").clicked() {
                state.clear_screenscraper_credentials();
            }
        });
    } else {
        ui.label(RichText::new("Not configured — the scraper plugin will refuse to run until this is filled in.").weak());
    }
    ui.add_space(6.0);
    egui::Grid::new("screenscraper_grid").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
        ui.label("Developer ID:");
        ui.text_edit_singleline(&mut state.screenscraper_dev_id_input);
        ui.end_row();
        ui.label("Developer password:");
        ui.add(egui::TextEdit::singleline(&mut state.screenscraper_dev_password_input).password(true));
        ui.end_row();
        ui.label("Software name:");
        ui.text_edit_singleline(&mut state.screenscraper_software_name_input);
        ui.end_row();
        ui.label("Account ID:");
        ui.text_edit_singleline(&mut state.screenscraper_user_id_input);
        ui.end_row();
        ui.label("Account password:");
        ui.add(egui::TextEdit::singleline(&mut state.screenscraper_user_password_input).password(true));
        ui.end_row();
    });
    if ui.button("Save ScreenScraper credentials").clicked() {
        state.save_screenscraper_credentials();
    }

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(12.0);

    ui.label(RichText::new("Shader associations").strong());
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Which shader preset to apply per RetroArch core, or per individual game. Run the \
             \"Shader Overrides Export\" plugin (Plugins tab) to write the actual RetroArch \
             override files from this list.",
        )
        .weak()
        .small(),
    );
    ui.add_space(6.0);
    if state.shader_presets.is_empty() {
        state.refresh_shader_presets();
    }
    let mut remove_assoc = None;
    for (index, assoc) in state.shader_associations.iter().enumerate() {
        ui.horizontal(|ui| {
            let target = match assoc.scope {
                retrotools_plugin_shaders::ShaderScope::System => format!("core: {}", assoc.core_name),
                retrotools_plugin_shaders::ShaderScope::Game => format!(
                    "game: {} / {} / {}",
                    assoc.core_name,
                    assoc.content_dir_name.as_deref().unwrap_or(""),
                    assoc.game_name.as_deref().unwrap_or("")
                ),
            };
            ui.label(format!("{target} → {}", assoc.preset));
            if ui.small_button("Remove").clicked() {
                remove_assoc = Some(index);
            }
        });
    }
    if let Some(index) = remove_assoc {
        state.remove_shader_association(index);
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.shader_new_is_game, "Per-game (unchecked = whole core)");
        ui.label("Core name:");
        ui.text_edit_singleline(&mut state.shader_new_core);
    });
    if state.shader_new_is_game {
        ui.horizontal(|ui| {
            ui.label("Content dir:");
            ui.text_edit_singleline(&mut state.shader_new_content_dir);
            ui.label("Game name:");
            ui.text_edit_singleline(&mut state.shader_new_game);
        });
    }
    ui.horizontal(|ui| {
        ui.label("Preset:");
        egui::ComboBox::from_id_source("shader_preset_combo")
            .selected_text(if state.shader_new_preset.is_empty() { "(choose)" } else { &state.shader_new_preset })
            .show_ui(ui, |ui| {
                let presets = state.shader_presets.clone();
                for preset in &presets {
                    ui.selectable_value(&mut state.shader_new_preset, preset.clone(), preset);
                }
            });
        if ui.button("Add association").clicked() {
            state.add_shader_association();
        }
    });

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(12.0);

    ui.label(RichText::new("RetroAchievements").strong());
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Your own RetroAchievements username and API key — never bundled with this app, \
             stored encrypted on this machine. Needed to sync the known-compatible-hash cache \
             used by the RetroAchievements plugin and the optional 1G1R tie-breaker.",
        )
        .weak()
        .small(),
    );
    ui.add_space(6.0);
    if state.config.retroachievements.is_configured() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("✓ Configured as {}", state.config.retroachievements.username)).color(egui::Color32::from_rgb(76, 175, 80)));
            if ui.small_button("Clear").clicked() {
                state.clear_retroachievements_credentials();
            }
        });
    } else {
        ui.label(RichText::new("Not configured — the RetroAchievements plugin will refuse to run until this is filled in.").weak());
    }
    ui.add_space(6.0);
    egui::Grid::new("retroachievements_grid").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
        ui.label("Username:");
        ui.text_edit_singleline(&mut state.retroachievements_username_input);
        ui.end_row();
        ui.label("API key:");
        ui.add(egui::TextEdit::singleline(&mut state.retroachievements_api_key_input).password(true));
        ui.end_row();
    });
    if ui.button("Save RetroAchievements credentials").clicked() {
        state.save_retroachievements_credentials();
    }

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(12.0);

    ui.label(RichText::new("Core compatibility database").strong());
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "A local, editable database mapping platform/game → recommended libretro core. \
             This project doesn't host or query any online service for this community data — \
             import a JSON file you already have (see docs/PLUGIN_DEV.md for the format).",
        )
        .weak()
        .small(),
    );
    ui.add_space(6.0);
    ui.label(format!("{} entrie(s) loaded", state.core_advisor_db.len()));
    if ui.button("Import core database file...").clicked() {
        if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
            state.import_core_advisor_database(&path);
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

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Update repository (owner/repo):");
        let mut repo = state.config.update_repository.clone().unwrap_or_default();
        if ui.text_edit_singleline(&mut repo).changed() {
            state.config.update_repository = if repo.trim().is_empty() { None } else { Some(repo) };
        }
        let can_check = state.config.update_repository.is_some() && !state.checking_for_updates;
        if ui.add_enabled(can_check, egui::Button::new("Check now")).clicked() {
            state.check_for_updates();
        }
        if state.checking_for_updates {
            ui.spinner();
        }
    });
    ui.label(
        RichText::new(
            "Checks GitHub's public Releases API for this repository — no auth token needed. \
             Leave empty to disable the check entirely.",
        )
        .weak()
        .small(),
    );
    if let Some(update) = &state.available_update {
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!("Update available: {}", update.version))
                .color(egui::Color32::from_rgb(76, 175, 80)),
        );
        ui.label(RichText::new(&update.download_url).weak().small());
    }

    ui.add_space(20.0);
    if ui.button(state.t(Key::SaveSettingsButton)).clicked() {
        match state.config.save() {
            Ok(()) => state.toast(ToastKind::Success, "Settings saved"),
            Err(err) => state.toast(ToastKind::Error, format!("Cannot save settings: {err}")),
        }
    }
}

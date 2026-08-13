use crate::i18n::Key;
use crate::state::{AppState, WizardStep};
use egui::{RichText, ScrollArea, Ui};

fn ordered_list_editor(ui: &mut Ui, id: &str, items: &mut Vec<String>, available: &std::collections::BTreeSet<String>) {
    let mut remove_index = None;
    let mut move_up = None;
    let mut move_down = None;

    for (index, item) in items.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("{}. {}", index + 1, item));
            if ui.small_button("^").clicked() && index > 0 {
                move_up = Some(index);
            }
            if ui.small_button("v").clicked() && index + 1 < items.len() {
                move_down = Some(index);
            }
            if ui.small_button("x").clicked() {
                remove_index = Some(index);
            }
        });
    }

    if let Some(index) = remove_index {
        items.remove(index);
    }
    if let Some(index) = move_up {
        items.swap(index, index - 1);
    }
    if let Some(index) = move_down {
        items.swap(index, index + 1);
    }

    let mut to_add: Option<String> = None;
    egui::ComboBox::from_id_source(id)
        .selected_text("Add...")
        .show_ui(ui, |ui| {
            for candidate in available {
                if !items.contains(candidate) && ui.selectable_label(false, candidate).clicked() {
                    to_add = Some(candidate.clone());
                }
            }
        });
    if let Some(value) = to_add {
        items.push(value);
    }
}

pub fn show(ui: &mut Ui, state: &mut AppState) {
    let Some(gameset) = state.current_gameset().cloned() else {
        ui.heading("1G1R Builder");
        ui.add_space(12.0);
        ui.label(RichText::new("Select a platform in the Platforms tab first.").weak());
        return;
    };

    ui.horizontal(|ui| {
        ui.heading(format!("1G1R Builder — {}", gameset.platform));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let label = state.t(Key::ExpertModeToggle);
            ui.checkbox(&mut state.expert_mode, label);
        });
    });
    ui.add_space(12.0);

    if state.expert_mode {
        ScrollArea::vertical().show(ui, |ui| {
            scan_section(ui, state);
            ui.add_space(16.0);
            ui.separator();
            ui.add_space(12.0);
            rules_section(ui, state, &gameset);
            ui.add_space(16.0);
            ui.separator();
            ui.add_space(12.0);
            preview_section(ui, state);
            ui.add_space(16.0);
            ui.separator();
            ui.add_space(12.0);
            build_section(ui, state);
        });
    } else {
        wizard(ui, state, &gameset);
    }
}

fn wizard(ui: &mut Ui, state: &mut AppState, gameset: &retrotools_core::GameSet) {
    ui.horizontal(|ui| {
        for step in [
            WizardStep::ChooseDat,
            WizardStep::Scan,
            WizardStep::Rules,
            WizardStep::Preview,
            WizardStep::Build,
        ] {
            let selected = state.wizard_step == step;
            if ui.selectable_label(selected, step.label()).clicked() {
                state.wizard_step = step;
            }
            ui.label("›");
        }
    });
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(12.0);

    ScrollArea::vertical().show(ui, |ui| match state.wizard_step {
        WizardStep::ChooseDat => {
            ui.label(RichText::new("Platform & DAT").strong().size(15.0));
            ui.add_space(6.0);
            ui.label(format!("Selected platform: {}", gameset.platform));
            ui.label(RichText::new("Go to the Platforms tab to change the active DAT.").weak());
        }
        WizardStep::Scan => scan_section(ui, state),
        WizardStep::Rules => rules_section(ui, state, gameset),
        WizardStep::Preview => preview_section(ui, state),
        WizardStep::Build => build_section(ui, state),
    });

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(state.wizard_step != WizardStep::ChooseDat, egui::Button::new("< Back"))
            .clicked()
        {
            state.wizard_step = state.wizard_step.previous();
        }
        if ui
            .add_enabled(state.wizard_step != WizardStep::Build, egui::Button::new("Next >"))
            .clicked()
        {
            state.wizard_step = state.wizard_step.next();
        }
    });
}

fn scan_section(ui: &mut Ui, state: &mut AppState) {
    ui.label(RichText::new("1. Scan").strong().size(15.0));
    ui.add_space(6.0);

    ui.horizontal(|ui| {
        if ui.button(state.t(Key::ChooseRomFolderButton)).clicked() {
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                state.rom_root = Some(folder);
            }
        }
        if let Some(root) = &state.rom_root {
            ui.label(root.display().to_string());
        } else {
            ui.label(RichText::new("No folder selected").weak());
        }
    });

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let can_scan = state.rom_root.is_some() && !state.scan_in_progress;
        if ui
            .add_enabled(can_scan, egui::Button::new(state.t(Key::StartScanButton)))
            .clicked()
        {
            if let Some(root) = state.rom_root.clone() {
                state.start_scan(root);
            }
        }
        if state.scan_in_progress {
            ui.spinner();
            if let Some(progress) = &state.scan_progress {
                let speed = state
                    .scan_speed()
                    .map(|(files_per_sec, bytes_per_sec)| {
                        format!(" — {files_per_sec:.1} files/s, {:.2} MB/s", bytes_per_sec / 1_000_000.0)
                    })
                    .unwrap_or_default();
                ui.label(format!(
                    "{}/{} file(s), {} bytes{speed}",
                    progress.files_scanned, progress.files_total, progress.bytes_scanned
                ));
            } else {
                ui.label("Starting...");
            }
        }
    });

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let mut watch_enabled = state.watch_folder_enabled;
        if ui.checkbox(&mut watch_enabled, "Watch folder for changes (auto re-scan)").changed() {
            state.set_watch_enabled(watch_enabled);
        }

        ui.add_space(12.0);
        ui.label("Auto-rescan every:");
        let selected_label = match state.auto_rescan_interval_secs {
            None => "Off".to_string(),
            Some(secs) => format!("{} min", secs / 60),
        };
        egui::ComboBox::from_id_source("auto_rescan_combo")
            .selected_text(&selected_label)
            .show_ui(ui, |ui| {
                if ui.selectable_label(false, "Off").clicked() {
                    state.auto_rescan_interval_secs = None;
                }
                for minutes in [1u64, 5, 15, 30] {
                    if ui.selectable_label(false, format!("{minutes} min")).clicked() {
                        state.auto_rescan_interval_secs = Some(minutes * 60);
                    }
                }
            });
    });

    if let Some(report) = &state.match_report {
        ui.add_space(8.0);
        ui.label(format!(
            "Matched: {}  Corrupt: {}  Unknown: {}  Missing: {}  Completion: {:.1}%",
            report.matched.len(),
            report.corrupt.len(),
            report.unknown.len(),
            report.missing.len(),
            report.completion_percent()
        ));

        let fix_report = retrotools_core::build_fix_report(report);
        if !fix_report.is_complete() {
            ui.add_space(6.0);
            egui::CollapsingHeader::new(format!("Fix report ({} action(s) needed)", fix_report.actions.len()))
                .show(ui, |ui| {
                    for action in &fix_report.actions {
                        let text = match action.kind {
                            retrotools_core::FixActionKind::Obtain => format!(
                                "[OBTAIN] {} — {} (expected CRC32 {})",
                                action.game_name,
                                action.rom_name,
                                action.expected_crc32.as_deref().unwrap_or("?")
                            ),
                            retrotools_core::FixActionKind::Replace => format!(
                                "[REPLACE] {} — {} (found CRC32 {})",
                                action.game_name,
                                action.rom_name,
                                action.current_crc32.as_deref().unwrap_or("?")
                            ),
                        };
                        ui.label(text);
                    }
                });
        }
    }
}

fn rules_section(ui: &mut Ui, state: &mut AppState, gameset: &retrotools_core::GameSet) {
    ui.label(RichText::new("2. Rules").strong().size(15.0));
    ui.add_space(6.0);

    ui.horizontal(|ui| {
        egui::ComboBox::from_id_source("preset_combo")
            .selected_text("Load preset...")
            .show_ui(ui, |ui| {
                for preset in retrotools_core::built_in_presets() {
                    if ui.selectable_label(false, &preset.name).clicked() {
                        state.load_profile(preset);
                    }
                }
            });

        let saved = state.saved_profiles();
        egui::ComboBox::from_id_source("saved_profile_combo")
            .selected_text("Load saved profile...")
            .show_ui(ui, |ui| {
                for profile in saved {
                    let label = profile.name.clone();
                    if ui.selectable_label(false, &label).clicked() {
                        state.load_profile(profile);
                    }
                }
            });

        if let Some(active) = &state.active_profile_name {
            ui.label(RichText::new(format!("Active: {active}")).weak());
        }
    });

    let mut save_name = state.active_profile_name.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("Save as:");
        let response = ui.text_edit_singleline(&mut save_name);
        if ui.button("Save profile").clicked() && !save_name.trim().is_empty() {
            state.save_profile(save_name.clone());
        }
        let _ = response;
    });

    ui.add_space(10.0);

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

    ui.columns(2, |columns| {
        columns[0].label(RichText::new("Region priority (top = preferred)").weak());
        ordered_list_editor(&mut columns[0], "region_order_add", &mut state.rules.region_order, &all_regions);

        columns[1].label(RichText::new("Language priority (top = preferred)").weak());
        ordered_list_editor(&mut columns[1], "language_order_add", &mut state.rules.language_order, &all_languages);
    });

    ui.add_space(10.0);
    ui.checkbox(&mut state.rules.prefer_parent, "Prefer parent release over clone (tie-break)");

    ui.add_space(6.0);
    ui.label(RichText::new("Exclude:").weak());
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut state.rules.exclude_beta, "Beta");
        ui.checkbox(&mut state.rules.exclude_proto, "Proto");
        ui.checkbox(&mut state.rules.exclude_demo, "Demo/Sample");
        ui.checkbox(&mut state.rules.exclude_kiosk, "Kiosk");
        ui.checkbox(&mut state.rules.exclude_promo, "Promo");
        ui.checkbox(&mut state.rules.exclude_unlicensed, "Unlicensed");
        ui.checkbox(&mut state.rules.exclude_pirate, "Pirate");
        ui.checkbox(&mut state.rules.exclude_bad_dump, "Bad Dump");
    });

    ui.add_space(10.0);
    if ui.button(state.t(Key::PreviewSelectionButton)).clicked() {
        state.run_preview();
    }
}

fn preview_section(ui: &mut Ui, state: &mut AppState) {
    ui.label(RichText::new("3. Preview (diff)").strong().size(15.0));
    ui.add_space(6.0);

    let Some(preview) = &state.selection_preview else {
        ui.label(RichText::new("Run a preview to see what would be kept/removed.").weak());
        return;
    };

    ui.label(format!(
        "{} kept, {} removed",
        preview.kept.len(),
        preview.removed.len()
    ));
    ui.add_space(6.0);

    ScrollArea::vertical().max_height(220.0).id_source("explanations_scroll").show(ui, |ui| {
        for explanation in &preview.explanations {
            ui.label(format!(
                "[{}] kept '{}' — {}",
                explanation.family, explanation.chosen, explanation.reason
            ));
        }
    });
}

fn build_section(ui: &mut Ui, state: &mut AppState) {
    ui.label(RichText::new("4. Build").strong().size(15.0));
    ui.add_space(6.0);

    ui.horizontal(|ui| {
        if ui.button("Choose destination...").clicked() {
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                state.rom_root.get_or_insert_with(|| folder.clone());
                state.last_build_summary = None;
                state.build_destination = Some(folder);
            }
        }
        if let Some(dest) = &state.build_destination {
            ui.label(dest.display().to_string());
        } else {
            ui.label(RichText::new("No destination selected").weak());
        }
    });

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_source("mode_combo")
            .selected_text(format!("{:?}", state.build_mode))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.build_mode, retrotools_core::TransferMode::Copy, "Copy");
                ui.selectable_value(&mut state.build_mode, retrotools_core::TransferMode::Move, "Move");
                ui.selectable_value(&mut state.build_mode, retrotools_core::TransferMode::HardLink, "Hardlink");
                ui.selectable_value(&mut state.build_mode, retrotools_core::TransferMode::SymLink, "Symlink");
            });

        egui::ComboBox::from_id_source("organize_combo")
            .selected_text(format!("{:?}", state.build_organize))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.build_organize, retrotools_core::OrganizeBy::Flat, "Flat");
                ui.selectable_value(&mut state.build_organize, retrotools_core::OrganizeBy::ByPlatform, "By platform");
                ui.selectable_value(
                    &mut state.build_organize,
                    retrotools_core::OrganizeBy::ByPlatformAndRegion,
                    "By platform + region",
                );
            });
    });

    ui.checkbox(&mut state.build_rename_to_dat_name, "Rename files to the DAT's canonical name");
    ui.checkbox(&mut state.build_dry_run, "Dry run (preview only, no filesystem change)");

    ui.add_space(8.0);
    let can_build = state.build_destination.is_some() && state.selection_preview.is_some() && !state.build_in_progress;
    if ui.add_enabled(can_build, egui::Button::new("Build 1G1R set")).clicked() {
        if let Some(dest) = state.build_destination.clone() {
            state.start_build(
                dest,
                state.build_mode,
                state.build_organize,
                state.build_rename_to_dat_name,
                state.build_dry_run,
            );
        }
    }
    if state.build_in_progress {
        ui.spinner();
    }
    if let Some(summary) = &state.last_build_summary {
        ui.label(summary);
    }
}

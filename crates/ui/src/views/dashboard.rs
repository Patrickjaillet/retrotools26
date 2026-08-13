use crate::i18n::Key;
use crate::state::AppState;
use egui::{RichText, Ui};

pub fn show(ui: &mut Ui, state: &AppState) {
    ui.heading(state.t(Key::DashboardTitle));
    ui.add_space(12.0);

    let platform_count = state.library.entries().len();
    let dat_count = state.library.entries().len();
    let games_total: usize = state.library.entries().iter().map(|e| e.gameset.games.len()).sum();
    let completion = state
        .match_report
        .as_ref()
        .map(|r| format!("{:.1}%", r.completion_percent()))
        .unwrap_or_else(|| "-".to_string());

    ui.horizontal_wrapped(|ui| {
        stat_card(ui, state.t(Key::DashboardPlatforms), &platform_count.to_string());
        stat_card(ui, state.t(Key::DashboardDatFilesLoaded), &dat_count.to_string());
        stat_card(ui, state.t(Key::DashboardGamesTracked), &games_total.to_string());
        stat_card(ui, state.t(Key::DashboardCompletion), &completion);
    });

    ui.add_space(20.0);
    ui.separator();
    ui.add_space(12.0);

    if let Some(platform) = &state.selected_platform {
        ui.label(RichText::new(format!("Active platform: {platform}")).strong());
        if let Some(report) = &state.match_report {
            ui.add_space(6.0);
            ui.label(format!(
                "Last scan: {} matched, {} corrupt, {} unknown, {} missing",
                report.matched.len(),
                report.corrupt.len(),
                report.unknown.len(),
                report.missing.len()
            ));
        } else {
            ui.add_space(6.0);
            ui.label(RichText::new("No scan performed yet for this platform.").weak());
        }
        if let Some(preview) = &state.selection_preview {
            ui.add_space(6.0);
            ui.label(format!(
                "1G1R preview: {} kept, {} removed",
                preview.kept.len(),
                preview.removed.len()
            ));
        }
    } else {
        ui.label(RichText::new(state.t(Key::DashboardNoActivity)).weak());
        ui.add_space(8.0);
        ui.label(RichText::new(state.t(Key::DashboardImportPrompt)).weak());
    }

    let stats = state.platform_stats();
    if !stats.is_empty() {
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(12.0);
        ui.label(RichText::new("Platforms overview").strong());
        ui.add_space(6.0);
        let mut entries: Vec<_> = stats.into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (platform, count) in entries {
            ui.label(format!("  {platform} — {count} game(s)"));
        }
    }
}

fn stat_card(ui: &mut Ui, label: &str, value: &str) {
    egui::Frame::group(ui.style())
        .rounding(egui::Rounding::same(10.0))
        .inner_margin(egui::Margin::same(16.0))
        .show(ui, |ui| {
            ui.set_min_width(160.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(value).size(26.0).strong());
                ui.label(RichText::new(label).weak());
            });
        });
}

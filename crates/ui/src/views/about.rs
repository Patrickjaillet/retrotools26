use egui::{RichText, Ui};
use retrotools_common::current_version;

pub fn show(ui: &mut Ui) {
    ui.heading("About Retro Tools 2026");
    ui.add_space(16.0);

    ui.label(RichText::new("Retro Tools 2026").size(20.0).strong());
    let version = current_version();
    ui.label(format!("Version {}", version));
    ui.add_space(12.0);

    egui::Grid::new("about_grid")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            ui.label(RichText::new("Copyright").weak());
            ui.label("© 2026 Patrick JAILLET — All rights reserved");
            ui.end_row();

            ui.label(RichText::new("License").weak());
            ui.label("MIT");
            ui.end_row();

            ui.label(RichText::new("Contact").weak());
            ui.hyperlink_to(
                "sandefjord.development@proton.me",
                "mailto:sandefjord.development@proton.me",
            );
            ui.end_row();

            ui.label(RichText::new("Website").weak());
            ui.hyperlink("https://patrickjaillet.github.io/retrotools26");
            ui.end_row();

            ui.label(RichText::new("Repository").weak());
            ui.hyperlink("https://github.com/Patrickjaillet/retrotools26");
            ui.end_row();
        });

    ui.add_space(20.0);
    ui.separator();
    ui.add_space(12.0);
    ui.label(RichText::new("Built with Rust and egui.").weak());
}

use egui::{Color32, Rounding, Stroke, Visuals};
use retrotools_common::config::ThemePreference;

pub fn apply_theme(ctx: &egui::Context, preference: ThemePreference, accent: [u8; 3]) {
    let mut visuals = match preference {
        ThemePreference::Light => Visuals::light(),
        ThemePreference::Dark => Visuals::dark(),
        ThemePreference::System => {
            if ctx.style().visuals.dark_mode {
                Visuals::dark()
            } else {
                Visuals::light()
            }
        }
    };

    let accent_color = Color32::from_rgb(accent[0], accent[1], accent[2]);
    let selection_text_color =
        if accent_color.r() as u32 + accent_color.g() as u32 + accent_color.b() as u32 > 384 {
            Color32::BLACK
        } else {
            Color32::WHITE
        };
    visuals.selection.bg_fill = accent_color;
    visuals.selection.stroke = Stroke::new(1.0_f32, selection_text_color);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, accent_color);
    visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, accent_color);
    visuals.window_rounding = Rounding::same(8.0);
    visuals.menu_rounding = Rounding::same(6.0);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.window_margin = egui::Margin::same(14.0);
    ctx.set_style(style);
}

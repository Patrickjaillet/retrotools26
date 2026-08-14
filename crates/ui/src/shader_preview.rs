/// Static visual reference for a shader preset, shown next to its name in
/// Settings. There is no real screenshot here on purpose: this project
/// doesn't host a bank of third-party images for every shader RetroArch
/// ships (and unlike the tiny `.slangp` pass-definition text the shader
/// library stores, a real "before/after" screenshot would typically be a
/// game frame someone else captured — its own separate copyright question).
/// Instead this draws a small synthetic reference image, letting the
/// preset's *category* (CRT/scanline/upscaler/unknown) pick a distinct,
/// illustrative pattern — categorized from the file name via
/// `PreviewKind::classify`, a pure function kept separate from drawing so
/// it's unit-testable without an `egui::Context`.
use egui::{Color32, Rect, Rounding, Ui, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewKind {
    Crt,
    Scanline,
    Upscaler,
    Generic,
}

impl PreviewKind {
    /// Classifies a preset file name by keyword — order matters, "crt"
    /// checked first since a CRT preset name may also mention "scanline".
    pub fn classify(preset_filename: &str) -> Self {
        let lower = preset_filename.to_lowercase();
        if lower.contains("crt") {
            PreviewKind::Crt
        } else if lower.contains("scanline") {
            PreviewKind::Scanline
        } else if lower.contains("scale")
            || lower.contains("hq")
            || lower.contains("edge")
            || lower.contains("xbr")
            || lower.contains("sabr")
        {
            PreviewKind::Upscaler
        } else {
            PreviewKind::Generic
        }
    }
}

const BASE_COLORS: [Color32; 4] = [
    Color32::from_rgb(214, 69, 65),
    Color32::from_rgb(69, 155, 214),
    Color32::from_rgb(232, 191, 63),
    Color32::from_rgb(96, 189, 104),
];

fn draw_base_pattern(painter: &egui::Painter, rect: Rect) {
    let cell_w = rect.width() / 4.0;
    for (i, color) in BASE_COLORS.iter().enumerate() {
        let cell = Rect::from_min_size(
            rect.min + Vec2::new(cell_w * i as f32, 0.0),
            Vec2::new(cell_w, rect.height()),
        );
        painter.rect_filled(cell, Rounding::ZERO, *color);
    }
}

fn draw_scanlines(painter: &egui::Painter, rect: Rect) {
    let mut y = rect.top();
    while y < rect.bottom() {
        painter.rect_filled(
            Rect::from_min_max(
                egui::pos2(rect.left(), y),
                egui::pos2(rect.right(), (y + 1.5).min(rect.bottom())),
            ),
            Rounding::ZERO,
            Color32::from_black_alpha(110),
        );
        y += 3.0;
    }
}

fn draw_vignette(painter: &egui::Painter, rect: Rect) {
    let corner = Vec2::new(rect.width() * 0.18, rect.height() * 0.28);
    for (dx, dy) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
        let anchor = egui::pos2(
            rect.left() + dx * rect.width(),
            rect.top() + dy * rect.height(),
        );
        let offset = Vec2::new(
            if dx > 0.0 { -corner.x } else { corner.x },
            if dy > 0.0 { -corner.y } else { corner.y },
        );
        painter.rect_filled(
            Rect::from_two_pos(anchor, anchor + offset),
            Rounding::ZERO,
            Color32::from_black_alpha(70),
        );
    }
}

fn draw_upscaler_hint(painter: &egui::Painter, rect: Rect) {
    // A coarse blocky half next to a smoother half, to suggest "before →
    // after" of a pixel-art upscaling filter.
    let mid = rect.left() + rect.width() * 0.5;
    let block = rect.width() / 12.0;
    let mut x = rect.left();
    let mut toggle = false;
    while x < mid {
        if toggle {
            painter.rect_filled(
                Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(block, rect.height())),
                Rounding::ZERO,
                Color32::from_black_alpha(60),
            );
        }
        x += block;
        toggle = !toggle;
    }
    painter.line_segment(
        [egui::pos2(mid, rect.top()), egui::pos2(mid, rect.bottom())],
        (1.5, Color32::WHITE),
    );
}

/// Draws a `size.x`x`size.y` static reference preview for a preset file
/// name at the current cursor position.
pub fn draw(ui: &mut Ui, preset_filename: &str, size: Vec2) {
    let (response, painter) = ui.allocate_painter(size, egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, Rounding::same(4.0), Color32::from_gray(20));
    draw_base_pattern(&painter, rect.shrink(1.0));

    match PreviewKind::classify(preset_filename) {
        PreviewKind::Crt => {
            draw_scanlines(&painter, rect);
            draw_vignette(&painter, rect);
        }
        PreviewKind::Scanline => draw_scanlines(&painter, rect),
        PreviewKind::Upscaler => draw_upscaler_hint(&painter, rect),
        PreviewKind::Generic => {}
    }
    painter.rect_stroke(rect, Rounding::same(4.0), (1.0, Color32::from_gray(60)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_crt_before_scanline_when_both_keywords_present() {
        assert_eq!(
            PreviewKind::classify("crt-geom-scanline.slangp"),
            PreviewKind::Crt
        );
    }

    #[test]
    fn classifies_known_shader_families() {
        assert_eq!(PreviewKind::classify("crt-geom.slangp"), PreviewKind::Crt);
        assert_eq!(
            PreviewKind::classify("scanlines-sharp.slangp"),
            PreviewKind::Scanline
        );
        assert_eq!(
            PreviewKind::classify("scale2x.slangp"),
            PreviewKind::Upscaler
        );
        assert_eq!(PreviewKind::classify("hq4x.slangp"), PreviewKind::Upscaler);
        assert_eq!(
            PreviewKind::classify("xbr-lv3.slangp"),
            PreviewKind::Upscaler
        );
    }

    #[test]
    fn falls_back_to_generic_for_an_unrecognized_name() {
        assert_eq!(
            PreviewKind::classify("my-custom-preset.slangp"),
            PreviewKind::Generic
        );
    }

    #[test]
    fn classification_is_case_insensitive() {
        assert_eq!(PreviewKind::classify("CRT-Geom.SLANGP"), PreviewKind::Crt);
    }
}

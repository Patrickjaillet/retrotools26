#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod i18n;
mod platform_badge;
mod shader_preview;
mod state;
mod theme;
mod toast;
mod views;

use retrotools_common::config::AppConfig;

fn main() -> eframe::Result<()> {
    let config = AppConfig::load().unwrap_or_default();
    let _log_guard = retrotools_common::logging::init_logging(&config.log_level);

    tracing::info!("starting Retro Tools 2026 {}", retrotools_common::current_version());

    // Raw 256x256 RGBA (no PNG container) generated alongside `assets/icon.ico`
    // (which is what actually shows up in Explorer/the taskbar, embedded into
    // the .exe by `build.rs`) — this one is only for the window's own
    // title-bar/Alt+Tab icon, and decoding it is just a byte-length check, no
    // image-parsing dependency needed.
    const ICON_RGBA: &[u8] = include_bytes!("../assets/icon_256.rgba");
    let icon = eframe::egui::IconData {
        rgba: ICON_RGBA.to_vec(),
        width: 256,
        height: 256,
    };

    let viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([960.0, 600.0])
        .with_title("Retro Tools 2026")
        .with_icon(icon);

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Retro Tools 2026",
        native_options,
        Box::new(|cc| Ok(Box::new(app::RetroToolsApp::new(cc)))),
    )
}

/// Headless UI smoke tests: drive every tab's `show()` against a real
/// `egui::Context` with no window and no `eframe` event loop — `egui`
/// supports running its layout/input pipeline purely in memory via
/// `Context::run`, which is exactly what this needs. `egui_kittest` (the
/// fuller-featured egui testing crate) isn't published for egui 0.28 (the
/// version this app is pinned to), so this is a small hand-rolled harness
/// instead of leaving "automated UI tests" completely undone.
#[cfg(test)]
mod ui_smoke_tests {
    use crate::state::AppState;
    use crate::views;
    use egui::{Context, Pos2, RawInput, Rect, Vec2};
    use retrotools_common::config::AppConfig;

    fn frame_input() -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            ..Default::default()
        }
    }

    fn run_frame(ctx: &Context, f: impl FnOnce(&mut egui::Ui)) {
        let mut f = Some(f);
        let _output = ctx.run(frame_input(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                (f.take().expect("run_frame's closure is called exactly once"))(ui);
            });
        });
    }

    #[test]
    fn every_tab_view_renders_without_panicking_across_several_frames() {
        let ctx = Context::default();
        let mut state = AppState::new(AppConfig::default());

        for _ in 0..3 {
            run_frame(&ctx, |ui| views::dashboard::show(ui, &state));
            run_frame(&ctx, |ui| views::platforms::show(ui, &mut state));
            run_frame(&ctx, |ui| views::games::show(ui, &mut state));
            run_frame(&ctx, |ui| views::onegameonerom::show(ui, &mut state));
            run_frame(&ctx, |ui| views::plugins::show(ui, &mut state));
            run_frame(&ctx, |ui| views::settings::show(ui, &mut state));
            run_frame(&ctx, views::about::show);
        }
    }

    /// Same idea, but with a real imported DAT and a selected platform, so
    /// the Games/1G1R tabs exercise their non-empty rendering paths (game
    /// list rows, grid cards, the 1G1R Expert-mode sections) instead of
    /// only the "nothing imported yet" placeholders.
    #[test]
    fn tab_views_render_with_a_selected_platform_and_games_loaded() {
        let ctx = Context::default();
        let mut state = AppState::new(AppConfig::default());

        let dat = r#"<?xml version="1.0"?>
<datafile><header><name>Smoke Test</name></header>
<game name="Smoke Game (Europe)"><rom name="Smoke Game (Europe).bin" size="1" crc="00000001"/></game>
<game name="Smoke Game (USA)"><rom name="Smoke Game (USA).bin" size="1" crc="00000002"/></game>
</datafile>"#;
        let dat_path = std::env::temp_dir().join(format!("rt26-ui-smoke-{}.dat", std::process::id()));
        std::fs::write(&dat_path, dat).unwrap();
        state.import_dat(dat_path.clone());
        std::fs::remove_file(&dat_path).ok();
        assert!(state.selected_platform.is_some(), "import_dat should select the imported platform");

        for _ in 0..2 {
            run_frame(&ctx, |ui| views::platforms::show(ui, &mut state));
            run_frame(&ctx, |ui| views::games::show(ui, &mut state));
            run_frame(&ctx, |ui| views::onegameonerom::show(ui, &mut state));
            state.expert_mode = !state.expert_mode;
            run_frame(&ctx, |ui| views::onegameonerom::show(ui, &mut state));
        }
    }
}

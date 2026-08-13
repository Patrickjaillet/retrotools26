use crate::i18n::{self, Key};
use crate::state::{AppState, ToastKind};
use crate::theme;
use crate::toast::ToastManager;
use crate::views;
use eframe::egui;
use egui::ScrollArea;
use retrotools_common::config::AppConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Tab {
    Dashboard,
    Platforms,
    Games,
    OneGameOneRom,
    Plugins,
    Settings,
    About,
}

impl Tab {
    fn label(&self, lang: i18n::Language) -> &'static str {
        let key = match self {
            Tab::Dashboard => Key::TabDashboard,
            Tab::Platforms => Key::TabPlatforms,
            Tab::Games => Key::TabGames,
            Tab::OneGameOneRom => Key::Tab1g1r,
            Tab::Plugins => Key::TabPlugins,
            Tab::Settings => Key::TabSettings,
            Tab::About => Key::TabAbout,
        };
        i18n::t(lang, key)
    }

    fn all() -> &'static [Tab] {
        &[
            Tab::Dashboard,
            Tab::Platforms,
            Tab::Games,
            Tab::OneGameOneRom,
            Tab::Plugins,
            Tab::Settings,
            Tab::About,
        ]
    }
}

/// An entry in the command palette (Ctrl+Shift+P). Deliberately a flat,
/// hand-written list rather than a plugin/registration system — the roadmap
/// asks for "raccourcis clavier complets + palette de commandes", not a
/// fully extensible command bus.
#[derive(Clone, Copy)]
enum Command {
    GoToTab(Tab),
    ImportDat,
    ToggleExpertMode,
    ToggleTheme,
}

impl Command {
    fn all() -> &'static [Command] {
        &[
            Command::GoToTab(Tab::Dashboard),
            Command::GoToTab(Tab::Platforms),
            Command::GoToTab(Tab::Games),
            Command::GoToTab(Tab::OneGameOneRom),
            Command::GoToTab(Tab::Plugins),
            Command::GoToTab(Tab::Settings),
            Command::GoToTab(Tab::About),
            Command::ImportDat,
            Command::ToggleExpertMode,
            Command::ToggleTheme,
        ]
    }

    fn label(&self, lang: i18n::Language) -> String {
        match self {
            Command::GoToTab(tab) => format!("Go to: {}", tab.label(lang)),
            Command::ImportDat => i18n::t(lang, Key::ImportDatButton).to_string(),
            Command::ToggleExpertMode => i18n::t(lang, Key::ExpertModeToggle).to_string(),
            Command::ToggleTheme => "Toggle light/dark theme".to_string(),
        }
    }
}

pub struct RetroToolsApp {
    active_tab: Tab,
    toasts: ToastManager,
    theme_applied: bool,
    applied_ui_scale: Option<f32>,
    state: AppState,
}

impl RetroToolsApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let config = AppConfig::load().unwrap_or_default();
        let check_on_startup = config.check_updates_on_startup;

        let mut state = AppState::new(config);
        if check_on_startup {
            state.check_for_updates();
        }

        Self {
            active_tab: Tab::Dashboard,
            toasts: ToastManager::default(),
            theme_applied: false,
            applied_ui_scale: None,
            state,
        }
    }

    fn top_bar(&mut self, ctx: &egui::Context) {
        let lang = self.state.language();
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Retro Tools 2026").strong().size(16.0));
                ui.separator();
                for tab in Tab::all() {
                    let selected = self.active_tab == *tab;
                    if ui.selectable_label(selected, tab.label(lang)).clicked() {
                        self.active_tab = *tab;
                    }
                }
                ui.separator();
                if ui.button(self.state.t(Key::ImportDatButton)).on_hover_text("Ctrl+O").clicked() {
                    self.import_dat_dialog();
                }
                if ui.button("\u{1F50D}").on_hover_text("Command palette (Ctrl+Shift+P)").clicked() {
                    self.state.command_palette_open = true;
                }
            });
            ui.add_space(6.0);
        });
    }

    fn import_dat_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("DAT files", &["dat", "xml", "zip"])
            .pick_file()
        {
            self.state.import_dat(path);
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        for path in dropped {
            let is_dat_like = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| matches!(e.to_lowercase().as_str(), "dat" | "xml" | "zip"))
                .unwrap_or(false);
            if is_dat_like {
                self.state.import_dat(path);
            } else {
                self.state.toast(
                    ToastKind::Warning,
                    format!("Unsupported file dropped: {}", path.display()),
                );
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let import_shortcut = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::O));
        if import_shortcut {
            self.import_dat_dialog();
        }

        let palette_shortcut = ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::P)
        });
        if palette_shortcut {
            self.state.command_palette_open = !self.state.command_palette_open;
            self.state.command_palette_query.clear();
            self.state.command_palette_selected = 0;
        }
        if self.state.command_palette_open && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.state.command_palette_open = false;
        }

        for (tab, key) in [
            (Tab::Dashboard, egui::Key::Num1),
            (Tab::Platforms, egui::Key::Num2),
            (Tab::Games, egui::Key::Num3),
            (Tab::OneGameOneRom, egui::Key::Num4),
            (Tab::Plugins, egui::Key::Num5),
            (Tab::Settings, egui::Key::Num6),
            (Tab::About, egui::Key::Num7),
        ] {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, key)) {
                self.active_tab = tab;
            }
        }
    }

    fn run_command(&mut self, command: Command) {
        match command {
            Command::GoToTab(tab) => self.active_tab = tab,
            Command::ImportDat => self.import_dat_dialog(),
            Command::ToggleExpertMode => self.state.expert_mode = !self.state.expert_mode,
            Command::ToggleTheme => {
                use retrotools_common::config::ThemePreference;
                self.state.config.theme = match self.state.config.theme {
                    ThemePreference::Dark => ThemePreference::Light,
                    _ => ThemePreference::Dark,
                };
                self.theme_applied = false;
            }
        }
        self.state.command_palette_open = false;
    }

    fn command_palette(&mut self, ctx: &egui::Context) {
        if !self.state.command_palette_open {
            return;
        }

        let lang = self.state.language();
        let query = self.state.command_palette_query.to_lowercase();
        let matches: Vec<Command> = Command::all()
            .iter()
            .copied()
            .filter(|c| query.is_empty() || c.label(lang).to_lowercase().contains(&query))
            .collect();
        if !matches.is_empty() {
            self.state.command_palette_selected = self.state.command_palette_selected.min(matches.len() - 1);
        }

        let mut still_open = true;
        let mut chosen = None;
        egui::Window::new("Command palette")
            .id(egui::Id::new("command_palette"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 80.0))
            .open(&mut still_open)
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                let response = ui.text_edit_singleline(&mut self.state.command_palette_query);
                response.request_focus();

                if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) && !matches.is_empty() {
                    self.state.command_palette_selected =
                        (self.state.command_palette_selected + 1) % matches.len();
                }
                if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && !matches.is_empty() {
                    self.state.command_palette_selected = self
                        .state
                        .command_palette_selected
                        .checked_sub(1)
                        .unwrap_or(matches.len() - 1);
                }
                let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));

                ui.add_space(6.0);
                ui.separator();
                ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                    for (index, command) in matches.iter().enumerate() {
                        let selected = index == self.state.command_palette_selected;
                        if ui.selectable_label(selected, command.label(lang)).clicked()
                            || (selected && enter_pressed)
                        {
                            chosen = Some(*command);
                        }
                    }
                });
            });

        if let Some(command) = chosen {
            self.run_command(command);
        } else {
            self.state.command_palette_open = still_open;
        }
    }

    fn drain_toasts(&mut self) {
        for pending in self.state.pending_toasts.drain(..) {
            match pending.kind {
                ToastKind::Info => self.toasts.info(pending.message),
                ToastKind::Success => self.toasts.success(pending.message),
                ToastKind::Warning => self.toasts.warning(pending.message),
                ToastKind::Error => self.toasts.error_message(pending.message),
            }
        }
    }

    fn central_content(&mut self, ctx: &egui::Context) {
        // Fade the tab body in on switch: `animate_bool_with_time` is keyed by
        // the active tab, so it restarts from 0 every time the tab changes and
        // settles back at 1 — a light "feedback visuel" without a real
        // animation/tween library.
        let fade_id = egui::Id::new(("tab_fade", self.active_tab));
        let opacity = ctx.animate_bool_with_time(fade_id, true, 0.15);
        if opacity < 1.0 {
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.scope(|ui| {
                ui.multiply_opacity(opacity);
                match self.active_tab {
                    Tab::Dashboard => views::dashboard::show(ui, &self.state),
                    Tab::Platforms => views::platforms::show(ui, &mut self.state),
                    Tab::Games => views::games::show(ui, &mut self.state),
                    Tab::OneGameOneRom => views::onegameonerom::show(ui, &mut self.state),
                    Tab::Plugins => views::plugins::show(ui, &mut self.state),
                    Tab::Settings => views::settings::show(ui, &mut self.state),
                    Tab::About => views::about::show(ui),
                }
            });
        });
    }
}

impl eframe::App for RetroToolsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.theme_applied {
            theme::apply_theme(ctx, self.state.config.theme, self.state.config.accent_color);
            self.theme_applied = true;
        }
        if self.applied_ui_scale != Some(self.state.config.ui_scale) {
            ctx.set_pixels_per_point(self.state.config.ui_scale);
            self.applied_ui_scale = Some(self.state.config.ui_scale);
        }

        self.state.poll_jobs();
        self.handle_dropped_files(ctx);
        self.handle_shortcuts(ctx);

        self.top_bar(ctx);
        self.central_content(ctx);
        self.command_palette(ctx);
        self.drain_toasts();
        self.toasts.show(ctx);

        if self.state.scan_in_progress || self.state.build_in_progress {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        } else if self.state.watch_folder_enabled || self.state.auto_rescan_interval_secs.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_secs(2));
        }
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        if let Err(err) = self.state.config.save() {
            tracing::warn!("failed to persist configuration: {}", err);
        }
    }
}

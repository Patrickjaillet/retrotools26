use egui_notify::Toasts;
use std::time::Duration;

#[derive(Default)]
pub struct ToastManager {
    toasts: Toasts,
}

impl ToastManager {
    pub fn info(&mut self, message: impl Into<String>) {
        self.toasts
            .info(message.into())
            .set_duration(Some(Duration::from_secs(3)));
    }

    pub fn success(&mut self, message: impl Into<String>) {
        self.toasts
            .success(message.into())
            .set_duration(Some(Duration::from_secs(3)));
    }

    pub fn warning(&mut self, message: impl Into<String>) {
        self.toasts
            .warning(message.into())
            .set_duration(Some(Duration::from_secs(5)));
    }

    pub fn error_message(&mut self, message: impl Into<String>) {
        self.toasts
            .error(message.into())
            .set_duration(Some(Duration::from_secs(6)));
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        self.toasts.show(ctx);
    }
}

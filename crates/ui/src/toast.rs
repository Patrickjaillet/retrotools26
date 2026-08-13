use egui_notify::Toasts;
use retrotools_common::error::{AppError, Severity};
use std::time::Duration;

pub struct ToastManager {
    toasts: Toasts,
}

impl Default for ToastManager {
    fn default() -> Self {
        Self {
            toasts: Toasts::default(),
        }
    }
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

    pub fn report(&mut self, error: &AppError) {
        match error.severity() {
            Severity::Info => self.info(error.user_message()),
            Severity::Warning => self.warning(error.user_message()),
            Severity::Error | Severity::Critical => self.error_message(error.user_message()),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        self.toasts.show(ctx);
    }
}

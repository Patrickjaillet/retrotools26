use retrotools_core::GameSet;
use std::path::{Path, PathBuf};

/// Everything a plugin needs to do its work, handed to it by the host
/// application. Plugins never touch `retrotools-core` internals directly
/// beyond the public types re-exposed here — they only see what the host
/// chooses to share.
pub struct PluginContext<'a> {
    /// The DAT currently loaded for the active platform (or, for a plugin
    /// that verifies something DAT-shaped that isn't ROMs — e.g. a BIOS
    /// pack — whatever DAT the caller passed in for that purpose).
    pub gameset: &'a GameSet,
    /// Names of the games the current 1G1R preview decided to keep, if a
    /// preview has been run. Empty when not applicable to the plugin.
    pub kept_game_names: &'a [String],
    /// A source directory to read from (e.g. a folder to scan/verify).
    pub source_dir: Option<&'a Path>,
    /// Where the plugin should write anything it produces.
    pub output_dir: &'a Path,
}

#[derive(Debug, Clone, Default)]
pub struct PluginOutcome {
    pub summary: String,
    pub files_written: Vec<PathBuf>,
}

pub type PluginResult<T> = Result<T, String>;

/// A unit of optional functionality that extends Retro Tools 2026 without
/// modifying `retrotools-core` or `retrotools-ui`. A plugin is a Rust crate
/// implementing this trait and registered with a [`PluginRegistry`] by the
/// host binary (CLI or UI) at startup — see `docs/PLUGIN_DEV.md`.
pub trait Plugin: Send + Sync {
    /// Stable, unique, machine-readable identifier (e.g. `"playlists"`).
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn run(&self, ctx: &PluginContext) -> PluginResult<PluginOutcome>;
}

#[derive(Default)]
pub struct PluginRegistry {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    pub fn plugins(&self) -> &[Box<dyn Plugin>] {
        &self.plugins
    }

    pub fn find(&self, id: &str) -> Option<&dyn Plugin> {
        self.plugins.iter().find(|p| p.id() == id).map(|p| p.as_ref())
    }

    pub fn run(&self, id: &str, ctx: &PluginContext) -> PluginResult<PluginOutcome> {
        self.find(id)
            .ok_or_else(|| format!("no plugin registered with id '{id}'"))?
            .run(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retrotools_core::{DatHeader, DatType};

    struct EchoPlugin;

    impl Plugin for EchoPlugin {
        fn id(&self) -> &'static str {
            "echo"
        }
        fn name(&self) -> &'static str {
            "Echo"
        }
        fn description(&self) -> &'static str {
            "Test plugin that reports how many games it saw."
        }
        fn run(&self, ctx: &PluginContext) -> PluginResult<PluginOutcome> {
            Ok(PluginOutcome {
                summary: format!("saw {} game(s)", ctx.gameset.games.len()),
                files_written: Vec::new(),
            })
        }
    }

    fn empty_gameset() -> GameSet {
        GameSet {
            platform: "Test".into(),
            dat_name: "Test".into(),
            dat_version: "1".into(),
            dat_type: DatType::Custom,
            header: DatHeader::default(),
            games: Vec::new(),
        }
    }

    #[test]
    fn registers_and_runs_a_plugin() {
        let mut registry = PluginRegistry::new();
        registry.register(Box::new(EchoPlugin));

        assert_eq!(registry.plugins().len(), 1);
        assert!(registry.find("echo").is_some());
        assert!(registry.find("missing").is_none());

        let gameset = empty_gameset();
        let kept = Vec::new();
        let output_dir = std::env::temp_dir();
        let ctx = PluginContext {
            gameset: &gameset,
            kept_game_names: &kept,
            source_dir: None,
            output_dir: &output_dir,
        };

        let outcome = registry.run("echo", &ctx).unwrap();
        assert_eq!(outcome.summary, "saw 0 game(s)");
    }

    #[test]
    fn running_an_unknown_plugin_id_fails() {
        let registry = PluginRegistry::new();
        let gameset = empty_gameset();
        let kept = Vec::new();
        let output_dir = std::env::temp_dir();
        let ctx = PluginContext {
            gameset: &gameset,
            kept_game_names: &kept,
            source_dir: None,
            output_dir: &output_dir,
        };
        assert!(registry.run("nope", &ctx).is_err());
    }
}

# Writing a Plugin

Retro Tools 2026 can be extended without touching `retrotools-core` or
`retrotools-ui`: a plugin is a Rust crate that implements the `Plugin` trait
from `retrotools-plugin-api` and is registered by the host binary (CLI or UI)
at startup.

## Why statically-linked crates instead of loadable `.dll`/`.so` plugins

A "real" plugin system often means dynamically loading third-party binaries
at runtime. That was deliberately **not** built here: dynamic loading on
Windows brings ABI-stability, versioning and code-signing problems that are
hard to get right, and loading arbitrary unsigned code into a desktop app is
a real attack surface. Instead, a plugin in this project is:

- a normal Rust crate living under `crates/` (or anywhere else, as a path or
  git dependency),
- that implements the `Plugin` trait,
- and is registered by adding one `registry.register(Box::new(MyPlugin))`
  call to the host binary's plugin registry (`crates/cli/src/main.rs`'s
  `build_plugin_registry()`, and `crates/ui/src/state.rs`'s
  `default_plugin_registry()`).

This keeps the plugin fully type-checked at compile time, gives it direct
access to `retrotools-core`'s types with no serialization boundary, and
avoids shipping/trusting foreign binaries — at the cost of requiring a
rebuild to add a plugin. If a true hot-loadable plugin system becomes a hard
requirement later, `PluginRegistry` is the seam where that would plug in
without changing how plugins themselves are written.

## The contract

```rust
pub struct PluginContext<'a> {
    pub gameset: &'a retrotools_core::GameSet,
    pub kept_game_names: &'a [String],
    pub source_dir: Option<&'a std::path::Path>,
    pub output_dir: &'a std::path::Path,
}

pub struct PluginOutcome {
    pub summary: String,
    pub files_written: Vec<std::path::PathBuf>,
}

pub trait Plugin: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn run(&self, ctx: &PluginContext) -> Result<PluginOutcome, String>;
}
```

- `gameset` is whatever DAT the caller loaded — usually the ROM DAT for the
  active platform, but a plugin can ask the user for a different kind of DAT
  (the bundled BIOS plugin expects a *BIOS* DAT here, not a ROM DAT).
- `kept_game_names` is the current 1G1R preview's "kept" list, if one has
  been computed; empty otherwise. Decide a sensible fallback when it's empty
  (the bundled playlist plugin falls back to every game in the DAT).
- `source_dir` / `output_dir` are folders the host's UI/CLI let the user
  pick; not every plugin needs `source_dir` (the playlist plugin doesn't —
  it errors out cleanly if a plugin that needs a folder wasn't given one,
  same as a normal `Result::Err`).
- `run` returns `Err(String)` on failure — that message is shown to the user
  as-is (in a CLI `eprintln!` or a UI toast), so make it actionable.

## Writing a new plugin, step by step

1. Create a new crate: `cargo new --lib crates/plugin-my-thing`.
2. Add it to the workspace `members` and `[workspace.dependencies]` in the
   root `Cargo.toml`, the same way `plugin-playlists` and `plugin-bios` are
   declared.
3. Depend on `retrotools-core` (for `GameSet`/`Game` etc.) and
   `retrotools-plugin-api` (for the trait).
4. Implement `Plugin` for a unit struct (or a struct holding configuration,
   if your plugin needs any):

   ```rust
   use retrotools_plugin_api::{Plugin, PluginContext, PluginOutcome, PluginResult};

   pub struct MyPlugin;

   impl Plugin for MyPlugin {
       fn id(&self) -> &'static str { "my-thing" }
       fn name(&self) -> &'static str { "My Thing" }
       fn description(&self) -> &'static str { "Does my thing." }

       fn run(&self, ctx: &PluginContext) -> PluginResult<PluginOutcome> {
           // ... do the work, write files under ctx.output_dir ...
           Ok(PluginOutcome {
               summary: "did the thing".into(),
               files_written: vec![],
           })
       }
   }
   ```
5. Register it in both host binaries so it shows up in the CLI and the UI:
   - `crates/cli/src/main.rs`, `build_plugin_registry()`
   - `crates/ui/src/state.rs`, `default_plugin_registry()`
6. Write unit tests against `PluginContext`/`PluginOutcome` directly — no UI
   or CLI harness needed; see `crates/plugin-playlists/src/lib.rs` and
   `crates/plugin-bios/src/lib.rs` for examples using an in-memory DAT
   (`retrotools_core::dat::parse_dat_str`) and a `tempdir`-style output
   folder.

## Running a plugin

From the CLI:

```bash
retrotools-cli plugin list
retrotools-cli plugin run <id> <dat-file> --output <dir> [--source <dir>] [--profile <name>]
```

From the UI: the **Plugins** tab lists every registered plugin with a `Run`
button; pick a source/output folder above the list first if the plugin you
want needs one.

## Bundled plugins as reference implementations

- **`retrotools-plugin-playlists`** (`crates/plugin-playlists`) — generates a
  RetroArch `.lpl`, a LaunchBox XML file and an ES-DE `gamelist.xml` from the
  current 1G1R selection. A good example of a plugin that only needs
  `gameset` + `kept_game_names` + `output_dir`.
- **`retrotools-plugin-bios`** (`crates/plugin-bios`) — verifies a folder of
  BIOS files against a BIOS DAT by delegating to `retrotools_core::scan` and
  `retrotools_core::match_scan`, the same functions the main 1G1R engine
  uses for ROMs. A good example of a plugin that reuses the core engine
  instead of reimplementing hashing/matching, and that needs `source_dir`.

## What's intentionally not shipped yet

`gestionnaire de médias/artwork avancé`, `scraper de métadonnées (IGDB,
ScreenScraper, TheGamesDB)`, `intégration RetroAchievements` and
`convertisseur de formats ROM/ISO (CHD, CSO, RVZ...)` from the roadmap are
not implemented as plugins in this pass: the first three need third-party
API keys/accounts this project doesn't have credentials for, and a correct
CHD/CSO/RVZ converter needs licensed or reverse-engineered codec support
well beyond what fits here. The `Plugin` trait and registry are ready for
them — writing one is exactly the process above.

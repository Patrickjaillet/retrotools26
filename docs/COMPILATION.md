# Compilation Guide

This document describes how to build Retro Tools 2026 from source.

## Prerequisites

- **Rust toolchain** (stable, 1.75 or newer — install via [rustup](https://rustup.rs))
- **Windows 10 (1809+) or Windows 11** as the target platform
- On Windows, the standard MSVC build tools (Visual Studio Build Tools, "Desktop development with C++" workload) are required by the default Rust toolchain

Verify your toolchain:

```bash
rustc --version
cargo --version
```

## Cloning the repository

```bash
git clone https://github.com/Patrickjaillet/retrotools26.git
cd retrotools26
```

## Building

Build every crate in the workspace in release mode:

```bash
cargo build --release --workspace
```

Build a single crate:

```bash
cargo build --release -p retrotools-ui
cargo build --release -p retrotools-cli
```

Release binaries are produced under `target/release/`:

- `retrotools2026.exe` — the graphical application
- `retrotools-cli.exe` — the command-line interface

## Running in development

```bash
cargo run -p retrotools-ui
cargo run -p retrotools-cli -- --help
```

## Checking code quality

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Workspace layout

```
retrotools26/
├── Cargo.toml              # Workspace manifest, centralized version
├── crates/
│   ├── common/              # Shared types: config, errors, logging, versioning
│   ├── core/                # DAT parsing, ROM scanning, 1G1R rule engine
│   ├── ui/                  # egui/eframe desktop application
│   ├── cli/                 # Command-line interface
│   ├── plugin-api/          # Plugin trait/registry contract (see docs/PLUGIN_DEV.md)
│   ├── plugin-playlists/    # Built-in plugin: RetroArch/LaunchBox/ES-DE playlist generator
│   └── plugin-bios/         # Built-in plugin: BIOS pack verification against a BIOS DAT
├── docs/                    # CHANGELOG, COMPILATION guide, PLUGIN_DEV guide, screenshots
├── resources/               # Bundled third-party CLI tools (see below)
└── .github/                 # CI workflows, issue and PR templates
```

## Bundled third-party tools (`resources/`)

RAR extraction and CHD (MAME Compressed Hunks of Data) support have no
maintained pure-Rust implementation, so `crates/core/src/archive.rs` shells
out to three external command-line tools instead of bundling a decoder:

| File | Used for | Project |
|---|---|---|
| `resources/UnRAR.exe` | Listing/extracting RAR archives (read-only) | [RARLab UnRAR](https://www.rarlab.com/rar_add.htm) — "UnRAR command line freeware" |
| `resources/chdman.exe` | Extracting CHD disk images | [MAME](https://www.mamedev.org/) (`chdman` utility) |
| `resources/7za.exe` | Fallback 7-Zip implementation (the pure-Rust `sevenz-rust` is used by default) | [7-Zip](https://www.7-zip.org/) standalone CLI (`7za`) |

Each of these ships under its own license, distinct from this project's MIT
license — `UnRAR.exe` in particular is "freeware" under RARLab's own terms
(free to redistribute unmodified alongside an application, but not to sell
standalone or modify). Before distributing a build, check each tool's
license terms and include their required notices; don't assume MIT coverage
extends to them.

`retrotools-core::external_tools::find_tool` locates each executable at
runtime, in order: an environment variable override
(`RETROTOOLS_UNRAR_PATH`, `RETROTOOLS_CHDMAN_PATH`, `RETROTOOLS_7ZA_PATH`), a
`resources/` folder next to the running executable (or a few levels above
it, so `cargo run`/`cargo test` also find the repo-root `resources/`
directory), then the system `PATH`. If none of these have the tool, RAR/CHD
scanning fails with a clear error rather than panicking — every other
format (ZIP, 7Z via `sevenz-rust`, TAR) keeps working regardless.

When packaging a release, copy `resources/` next to `retrotools2026.exe`
and `retrotools-cli.exe` under `target/release/` (or wherever the installer
places them) so the lookup above finds them.

### RVZ (GameCube/Wii) and CSO (PSP) conversion — not bundled

`convert to-rvz`/`from-rvz` and `convert to-cso`/`from-cso` (`crates/core/src/convert.rs`)
shell out the same way, but to two tools this project does **not** bundle
under `resources/` (unlike UnRAR/chdman/7za above) since neither ships a
freely-redistributable standalone CLI build this project could legally
include:

| Tool | Used for | Project | Env var override |
|---|---|---|---|
| `DolphinTool.exe` | RVZ conversion | [Dolphin Emulator](https://dolphin-emu.org/) — ships as part of a Dolphin install | `RETROTOOLS_DOLPHINTOOL_PATH` |
| `maxcso.exe` | CSO conversion | [maxcso](https://github.com/unknownbrackets/maxcso) | `RETROTOOLS_MAXCSO_PATH` |

Install either tool yourself and either put it on `PATH`, set the matching
environment variable, or drop it into your own `resources/` folder next to
the executable (`find_tool` checks that location too, it doesn't care
whether this project or the user put a file there). Without the tool
present, the corresponding `convert` subcommand fails with a clear error
naming what's missing — CHD conversion and every scanning format keep
working regardless.

## Packaging (Windows installer & portable build)

Two distribution forms are produced from the same release build, via
scripts under `packaging/`:

### Installer (Inno Setup)

Requires [Inno Setup 7](https://www.innosetup.com) (`ISCC.exe`, the
command-line compiler). Not installed by default — install it once
(`choco install innosetup` or the official installer), then:

```powershell
cargo build --workspace --release
& "C:\Program Files\Inno Setup 7\ISCC.exe" packaging\installer.iss
```

Produces `packaging\output\RetroTools2026-Setup-<version>.exe`: a
per-user installer (no admin rights required — `PrivilegesRequired=lowest`)
that copies `retrotools2026.exe`, `retrotools-cli.exe`, `resources\*.exe`,
and the docs, and creates Start Menu/optional desktop shortcuts. It has
been verified end-to-end on this project's dev machine with a real silent
install/launch/uninstall cycle (`/VERYSILENT`), not just a successful
compile.

The installer is **unsigned** — no code-signing certificate is available in
this environment, so Windows SmartScreen will warn on first run until
either a certificate is obtained or the binary builds enough download
reputation. That's a real, currently-unresolved gap, not an oversight.

### Portable build

```powershell
cargo build --workspace --release
powershell -ExecutionPolicy Bypass -File packaging\make_portable.ps1
```

Produces `packaging\output\RetroTools2026-Portable-<version>.zip`: the same
binaries and `resources\` folder, plus a `portable.txt` marker file. Its
mere presence next to `retrotools2026.exe` is what
`retrotools_common::config::is_portable_mode()` checks — when found, every
config/cache/log/DAT-library path resolves under `<exe_dir>\data\` instead
of the per-user profile, so the whole folder can be moved, copied to a USB
stick, or deleted without leaving anything behind on the host machine.
Verified by actually running the packaged exe and confirming
`data\config.toml` and `data\logs\` appear next to it.

### Windows version compatibility

Developed and tested on Windows 11 (build 10.0.26100). The code has no
Windows-11-specific API calls (`eframe`/`egui`'s `glow` backend, `std::fs`,
and `directories`/`ProjectDirs` are all supported back to Windows 7+), so it
should run unmodified on Windows 10 (1809+) — but that has **not** been
independently verified on an actual Windows 10 machine or VM in this
environment; only Windows 11 has.

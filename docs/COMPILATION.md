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

## Packaging (Windows installer)

Installer packaging (MSI/EXE via WiX or Inno Setup) is introduced in a later development phase and will be documented here once available. It should copy `resources/` alongside the installed binaries.

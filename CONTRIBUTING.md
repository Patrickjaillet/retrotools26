# Contributing to Retro Tools 2026

Thank you for your interest in contributing. This document describes the rules and conventions that keep the codebase consistent and professional.

## Ground Rules

- **Language:** all code, comments in commit messages, documentation, issues and pull requests must be written in English.
- **No inline comments in source code.** Code should be self-explanatory through clear naming and small, focused functions. Use documentation comments (`///`) sparingly only where public API behavior genuinely needs clarification.
- **Target platforms:** Windows 10 (build 1809+) and Windows 11. Avoid platform-specific code that would break Windows compatibility.
- **Versioning:** the project follows strict [Semantic Versioning](https://semver.org/). The version is centralized in the workspace `Cargo.toml` — do not hardcode version strings elsewhere.
- Every user-facing change must be reflected in [`docs/CHANGELOG.md`](docs/CHANGELOG.md) under an `[Unreleased]` section.

## Development Workflow

1. Fork the repository and create a feature branch from `main`.
2. Make your changes, following the code style enforced by `rustfmt` and `clippy`.
3. Run the full check suite before opening a pull request:

   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```

4. Update [`docs/CHANGELOG.md`](docs/CHANGELOG.md) with your changes.
5. Open a pull request using the provided template, describing the change and its motivation.

## Commit Messages

Use clear, imperative commit messages (e.g. `Add region priority editor to 1G1R rule panel`). Group related changes into a single commit where possible.

## Code Style

- Follow idiomatic Rust and the project's existing module structure (`common`, `core`, `ui`, `cli`).
- Prefer explicit error handling (`AppResult<T>`) over panics in library code.
- Keep UI code (crate `ui`) free of business logic; business logic belongs in `core`.
- New functionality that could be reused outside the 1G1R workflow should be designed as a candidate plugin module (see the extensibility architecture in the project roadmap).

## Reporting Issues

Please use the issue templates under `.github/ISSUE_TEMPLATE/` when reporting bugs or requesting features, and include as much detail as possible (OS version, steps to reproduce, expected vs. actual behavior).

## Code of Conduct

Participation in this project is governed by the [Code of Conduct](CODE_OF_CONDUCT.md).

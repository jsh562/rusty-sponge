# Changelog

All notable changes to `rusty-sponge` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- CLI binary `rusty-sponge`: soak up all of stdin and write it atomically to a file (Rust port of moreutils `sponge`).
- Atomic in-place file rewrite via sibling-directory tempfile + atomic rename, preserving Unix `st_mode` bits and Windows read-only attribute.
- Pass-through to stdout when no file argument is given (pipeline batching).
- Append mode (`-a`) that reads the existing file contents into the buffer before stdin, then atomically replaces the target.
- Hybrid in-memory + tempfile-spill buffer engine: in-memory `Vec<u8>` up to a 128 MiB default threshold, then spills to a sibling tempfile so arbitrarily large inputs do not exhaust RAM.
- Configurable spill threshold via the `RUSTY_SPONGE_SPILL_MB` env var (Default mode) or `SpongeBuilder::spill_threshold` (library).
- Signal-driven cleanup: SIGINT / SIGTERM / SIGHUP on Unix, `CTRL_C_EVENT` / `CTRL_BREAK_EVENT` / `CTRL_CLOSE_EVENT` on Windows — tempfile is removed before exit, target file is never left in a partial state.
- Strict moreutils-compatibility mode via the `--strict` flag, the `RUSTY_SPONGE_STRICT` env var, or invocation as `sponge` (via the `sponge-alias` cargo feature, a symlink, or a shell alias). In Strict mode, Rusty-only flags are rejected with stderr diagnostics byte-equal to moreutils' own usage layout.
- Optional `sponge` binary alias, gated behind the `sponge-alias` cargo feature. Default `cargo install rusty-sponge` installs only `rusty-sponge`; `cargo install rusty-sponge --features sponge-alias` adds the moreutils-name alias.
- `completions <shell>` subcommand emitting shell-completion scripts for bash, zsh, fish, and PowerShell.
- Public Rust library API: `SpongeBuilder` (with `#[must_use]` chain methods and validation at `build()`) → `Sponge::run(impl Read)` as the byte-typed canonical surface.
- Library-without-binary build: `default-features = false` drops `clap`, `clap_complete`, `anyhow`, and `signal-hook` from the dependency closure.
- README Compatibility Matrix at `docs/COMPATIBILITY.md`, generated from the canonical CLI definition and drift-tested in CI on every PR.
- Cross-platform binary distribution: Linux x86_64/aarch64, macOS x86_64/aarch64, Windows x86_64 via `cargo-binstall` metadata pointing at GitHub Release archives.

### Behavioral parity with moreutils — verified byte-equal

Snapshot tests under `tests/compat_default.rs` and `tests/compat_strict.rs` assert byte-equal output against captured moreutils `sponge` output for: happy-path replacement (empty / small text / binary / large); existing-target replacement; symlink-target write-through; missing-target creation; target-is-directory error; `-a` with existing + missing target; Strict-mode `--help` rejection; Strict-mode `-h` usage line; Strict-mode unknown-option error.

Fixtures captured under pinned `LC_ALL=C.UTF-8` from the moreutils `sponge.c` source at a recorded upstream commit (capture protocol in `fixtures/README.md`).

### MSRV

Minimum supported Rust version: **1.85**.

This is an explicit upward deviation from the Rusty portfolio's standard "current stable minus two minor releases" rule, forced by Rust edition 2024 (which requires 1.85+). The portfolio rule remains in effect for ports not using edition 2024; this crate's MSRV will advance with edition adoption rather than with the rolling N-2 cadence.

### Known limitations at v0.1.0

- **Atomic-safety scope** (FR-006): the guarantee applies only to the regular-file atomic-rename path. The symlink/reparse write-through path (FR-010) and the cross-volume fallback path (FR-025) explicitly forgo it, matching moreutils. See the Compatibility statement in [`README.md`](README.md).
- **Symlink semantics divergence**: on Windows, NTFS reparse points (symbolic links, junctions, mount points) are treated as non-regular and trigger the write-through path consistently. POSIX symlinks behave identically. Documented divergence.
- **Spill threshold is static (128 MiB default)** rather than moreutils' dynamic ½-available-RAM. Configurable via `RUSTY_SPONGE_SPILL_MB` (Default mode) or the library builder.
- **`sponge-alias` cargo feature** ships a second binary named `sponge`. Users with moreutils already installed may experience PATH-order conflicts — by design, the user chooses which `sponge` wins via their PATH.

### Verified

- Tests passing on Rust 1.85 (MSRV) and current stable.
- Clippy strict (`-D warnings`) clean.
- rustfmt clean.
- `cargo audit` clean.
- Library API consumable with `default-features = false`.

### Compatibility statement

A full Compatibility Matrix mapping every moreutils `sponge` flag and every Rusty-added flag to Default-mode and Strict-mode behavior lives at [`docs/COMPATIBILITY.md`](docs/COMPATIBILITY.md). The file is generated from the canonical CLI definition and CI fails on drift.

[Unreleased]: https://github.com/jsh562/rusty-sponge/compare/v0.0.0...HEAD

# Changelog

All notable changes to `rusty-sponge` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-05-25

### Added (additive only — no v0.1.x behavior changed)

- Portfolio-wide [Cargo Features Convention](https://github.com/jsh562/rustylib/blob/main/specs/adrs/0006-cargo-features-convention-for-portfolio-ports.md)
  layout per ADR-0006 + `project-instructions.md` §Cargo Feature Surface. rusty-sponge applies the minimum convention as a **single-capability port** per spec 00011 §Scope Edge Cases.
- New umbrella features (all `["cli"]` composition for this single-cap port):
  - `full` — kitchen-sink umbrella per FR-002
  - `sponge-classic` — required `<port>-classic` umbrella per FR-004; moreutils `sponge` drop-in replacement
  - `sponge-minimal` — preset bundle per FR-007; explicit minimal-CLI semantic alias
- `default` now aliases to `full` instead of directly to `cli`. Resolved dependency set is identical (`full = ["cli"]`); no observable change for any consumer.
- See [`docs/feature-layout.md`](docs/feature-layout.md) for the zero-leaf rationale.

All v0.1.x feature names are preserved verbatim with identical compositions. `cli = ["dep:clap", "dep:clap_complete", "dep:anyhow", "dep:signal-hook"]` is unchanged. `sponge-alias = ["cli"]` is unchanged. `bench = ["dep:criterion"]` is unchanged. Library consumers using `default-features = false` get the same CLI-stripped build. Users running `cargo install rusty-sponge --features sponge-alias` continue to work unchanged. Users running `cargo bench --features bench` continue to work unchanged.

### Notes

- See the new `## Cargo Features` section in `README.md` for the
  feature matrix, preset bundles, keep-list workaround, and convention
  authority citations.
- Reference: [ADR-0006](https://github.com/jsh562/rustylib/blob/main/specs/adrs/0006-cargo-features-convention-for-portfolio-ports.md)
  (why this layout) + [`project-instructions.md` §Cargo Feature Surface](https://github.com/jsh562/rustylib/blob/main/project-instructions.md)
  (what the rules are).
- CI matrix expanded per spec 00011 FR-010..FR-014: now includes
  `test-default` (kitchen sink + cross-compile), `test-no-default`
  (bare library + dep-tree audit per SC-001), `test-sponge-classic`,
  `test-sponge-minimal` (preset bundles per SC-003), `test-keeplist`
  (keep-list workaround per SC-004), and `lint-convention` (vendored
  `tools/feature-lint/run.sh` invocation per FR-052). Tier 4
  (`check-leaf-<leaf>`) is intentionally empty — zero leaves carved
  per docs/feature-layout.md.
- The lint script is **vendored** into `tools/feature-lint/` (synced
  from the umbrella `jsh562/rustylib` repo) so per-port CI workflows
  do not depend on cross-repo `actions/checkout` of the private
  umbrella. Sync precedent set by rusty-figlet v0.2.0 (E011 Phase 2
  iteration 6) and rusty-ts v0.2.0 (E011 Phase 3).

## [0.1.0] - 2026-05-23

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

[Unreleased]: https://github.com/jsh562/rusty-sponge/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/jsh562/rusty-sponge/releases/tag/v0.2.0
[0.1.0]: https://github.com/jsh562/rusty-sponge/releases/tag/v0.1.0

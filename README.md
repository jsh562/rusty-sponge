# rusty-sponge

A Rust port of the moreutils `sponge` utility: soak up all of stdin and write it atomically to a file, so you can safely do `cmd file | rusty-sponge file` without the shell-truncation race that breaks the equivalent `cmd file > file`. Static binaries on Linux, macOS, and Windows; works with or without a Rust toolchain via `cargo install` or `cargo binstall`. Default mode adds a few niceties moreutils doesn't have (`--help`, `--version`, `completions`, `RUSTY_SPONGE_SPILL_MB` env override); Strict mode reverts every observable surface to byte-identical moreutils behavior for drop-in migration.

Part of the [Rusty portfolio](https://jsh562.github.io/rusty-portfolio) — a collection of small Rust ports of utilities missing from the Rust ecosystem.

## Install

### With a Rust toolchain

```sh
cargo install rusty-sponge
```

To also install the `sponge` binary alias (auto-enables Strict mode on invocation):

```sh
cargo install rusty-sponge --features sponge-alias
```

### Without a Rust toolchain (prebuilt binaries via cargo-binstall)

```sh
cargo binstall rusty-sponge
```

### Direct download

Per-target archives are attached to each [GitHub Release](https://github.com/jsh562/rusty-sponge/releases). Linux x86_64/aarch64, macOS x86_64/aarch64, Windows x86_64. Each archive contains the binary plus pre-generated shell-completion scripts for bash, zsh, fish, and PowerShell.

## Usage

```sh
# In-place file rewrite (sponge's headline use case)
sort file.txt | rusty-sponge file.txt

# Pipeline batching to stdout (no file argument)
producer | rusty-sponge | consumer

# Append mode (read existing file first, then append stdin, then atomically replace)
echo "new line" | rusty-sponge -a logfile

# Strict moreutils-compat mode (rejects `--help`/`--version`/`completions`, mirrors stderr layout)
some-command | rusty-sponge --strict file
RUSTY_SPONGE_STRICT=1 some-command | rusty-sponge file
some-command | sponge file    # via the sponge-alias feature or a symlink — argv[0] auto-detect

# Configurable spill threshold (Default mode only; default 128 MiB)
RUSTY_SPONGE_SPILL_MB=8 huge-producer | rusty-sponge target.bin

# Shell completions
rusty-sponge completions bash    # > ~/.bash_completion.d/rusty-sponge
rusty-sponge completions zsh     # > ~/.zfunc/_rusty-sponge
rusty-sponge completions fish    # > ~/.config/fish/completions/rusty-sponge.fish
rusty-sponge completions powershell
```

## Compatibility statement (vs moreutils sponge)

Byte-level fidelity is verified by snapshot tests against captured moreutils-`sponge` output under a pinned environment (`LC_ALL=C.UTF-8`). The snapshot reference is moreutils at a pinned upstream commit recorded in [`fixtures/README.md`](fixtures/README.md).

**Atomic-safety guarantee (FR-006)**: When `rusty-sponge` writes to a regular non-symlink file, it writes to a sibling tempfile in the target's parent directory and atomically `rename`s into place. Mid-write failures (SIGKILL, power loss, disk full) leave the original file byte-identical to its pre-invocation state — this is the property the original `sponge` was invented to provide. **The guarantee does NOT apply when**:
1. The target is a symlink or non-regular file (FR-010) — the linked file is written through with `O_WRONLY+O_TRUNC`, matching moreutils' `S_ISREG && !S_ISLNK` short-circuit.
2. The cross-volume / shared-handle atomic-rename fallback path triggers (FR-025) — non-atomic copy + truncate-and-rewrite is used as a last resort.
Both fallback paths match moreutils behavior; they are documented limitations, not bugs.

**Documented intentional divergences from moreutils sponge** (also enumerated in [`docs/COMPATIBILITY.md`](docs/COMPATIBILITY.md) — generated from the CLI definition and drift-tested in CI):

1. **`--help` / `--version` flags**: not present in moreutils. Default-mode additions; rejected in Strict mode.
2. **`completions` subcommand**: not present in moreutils. Default-mode addition; rejected in Strict mode.
3. **`RUSTY_SPONGE_SPILL_MB` env var**: not defined by moreutils (which sizes its spill heuristic dynamically from available RAM). Honored in Default mode; ignored in Strict mode.
4. **Spill threshold default**: 128 MiB (compile-time constant) vs moreutils' dynamic ½-available-RAM. Trades RAM-aware sizing for predictability; configurable via the env var or library builder.

In Strict mode, exit codes, stderr diagnostic text, and the `-h` usage layout match moreutils. See [`docs/COMPATIBILITY.md`](docs/COMPATIBILITY.md) for the full per-flag matrix and exit-code table.

## Library API

The crate exposes a public Rust API for programmatic use. The canonical surface is byte-typed (preserves non-UTF-8 payload bytes per FR-012); the builder produces a `Sponge` runtime that owns the buffer and the atomic-rename procedure.

```rust
use rusty_sponge::{SpongeBuilder, Target, CompatibilityMode};
use std::io::Cursor;
use std::path::PathBuf;

let mut sponge = SpongeBuilder::new()
    .target(Target::File(PathBuf::from("output.txt")))
    .append(false)
    .spill_threshold(64 * 1024 * 1024)
    .compat(CompatibilityMode::Default)
    .build()?;

sponge.run(Cursor::new(b"hello\nworld\n"))?;
# Ok::<(), rusty_sponge::Error>(())
```

To use the library without pulling in the CLI dependencies:

```toml
[dependencies]
rusty-sponge = { version = "0.1", default-features = false }
```

### Stability commitment

**Lockstep SemVer**: the library and binary share a single crate version. Within the `0.x` series, minor version bumps may introduce breaking changes per standard Cargo semantics — pin to the patch version (`= "0.1.0"`) if breakage is a concern. Once `1.0` lands, the API is frozen to additive-only changes guarded by `#[non_exhaustive]` on every public enum and struct.

## MSRV

Minimum supported Rust version: **1.85**.

This is an upward deviation from the Rusty portfolio's standard "current stable minus two minor releases" rule, forced by the crate's use of Rust edition 2024 (which requires 1.85+). The portfolio rule remains in effect for ports not using edition 2024; this crate's MSRV will advance with edition adoption, not with the rolling N-2 cadence.

## Relationship to moreutils

`rusty-sponge` is a **clean-room Rust reimplementation** of the moreutils `sponge` utility. It contains **no source code from moreutils** — only a from-scratch Rust implementation that observes the documented behavior of moreutils `sponge` and reproduces it.

The moreutils `sponge` C source is © Colin Watson and Tollef Fog Heen (2006) and licensed under the GNU GPL (v2 or later). That license governs the *C source code*. Behavioral interfaces (flag set, buffering semantics, atomic-rename pattern) are not copyrightable, so a clean-room reimplementation under a different license is well-established practice — the same posture as [`uutils/coreutils`](https://github.com/uutils/coreutils) (MIT-licensed reimplementation of GPL-licensed GNU coreutils).

`rusty-sponge` does **not** distribute or derive from the moreutils source code. Snapshot tests in this repository compare `rusty-sponge` *runtime output* against captured *moreutils sponge runtime output* (captured by running moreutils against fixtures and recording bytes) — that is not source-code derivation either. The captured output bytes are facts, not creative expression.

If you want the original moreutils `sponge`, install it via your platform's package manager (`apt install moreutils`, `brew install moreutils`, etc.) — that is unaffected by this port's existence.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE](LICENSE))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

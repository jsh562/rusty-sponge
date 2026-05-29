# rusty-sponge

Soak up stdin & rewrite the file atomically. Rust port of moreutils [`sponge(1)`](https://joeyh.name/code/moreutils/).

[![crates.io](https://img.shields.io/crates/v/rusty-sponge.svg)](https://crates.io/crates/rusty-sponge)
[![docs.rs](https://docs.rs/rusty-sponge/badge.svg)](https://docs.rs/rusty-sponge)
[![CI](https://github.com/jsh562/rusty-sponge/actions/workflows/ci.yml/badge.svg)](https://github.com/jsh562/rusty-sponge/actions/workflows/ci.yml)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue.svg)](#msrv)
[![license: MIT OR Apache-2.0](https://img.shields.io/crates/l/rusty-sponge.svg)](#license)

Lets you write `cmd file | rusty-sponge file` without the shell-truncation race that breaks the equivalent `cmd file > file`. Default mode adds `--help`, `--version`, `completions`, & a `RUSTY_SPONGE_SPILL_MB` env override. Strict mode reverts every observable surface to byte-equal moreutils `sponge` for drop-in migration.

Part of the [Rusty portfolio](https://jsh562.github.io/rusty-portfolio).

## Install

```sh
cargo install rusty-sponge
# or, with prebuilt binaries:
cargo binstall rusty-sponge
# or, download directly from GitHub Releases:
# https://github.com/jsh562/rusty-sponge/releases
```

To also install a `sponge` binary alias (argv[0] auto-detect routes into Strict mode):

```sh
cargo install rusty-sponge --features sponge-alias
```

## Usage

```sh
# In-place rewrite without the shell-truncation race (sponge's headline use case)
sort file.txt | rusty-sponge file.txt

# Filter a config file & write it back safely
grep -v '^#' config.yaml | rusty-sponge config.yaml

# Pipeline batching to stdout (no file argument; useful as a flow barrier)
producer | rusty-sponge | consumer

# Append mode (read existing file first, then append stdin, then atomic rename)
echo "new line" | rusty-sponge -a logfile

# Configurable spill threshold (Default mode only; default 128 MiB)
RUSTY_SPONGE_SPILL_MB=8 huge-producer | rusty-sponge target.bin

# Strict moreutils-compat mode (drop-in moreutils sponge replacement)
some-command | rusty-sponge --strict file
RUSTY_SPONGE_STRICT=1 some-command | rusty-sponge file
some-command | sponge file               # via sponge-alias feature or argv[0] symlink

# Shell completions
rusty-sponge completions bash             # > ~/.bash_completion.d/rusty-sponge
rusty-sponge completions zsh              # > ~/.zfunc/_rusty-sponge
rusty-sponge completions fish             # > ~/.config/fish/completions/rusty-sponge.fish
rusty-sponge completions powershell
```

## Library API

The crate exposes a byte-typed runtime. The builder owns the buffer & the atomic-rename procedure. Use it inside a daemon when you want sponge's crash-safety without spawning a child process per write.

```rust,no_run
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

For library-only consumers without CLI deps see the [Cargo Features](#cargo-features) section.

## Cargo Features

`default` enables `full`, which (for this single-capability port) resolves to the `cli` umbrella. `sponge-classic` reproduces v0.1.x bare-port behavior matching upstream moreutils `sponge` 1:1. To strip the CLI surface use `default-features = false` or `--no-default-features` & add the features you want.

rusty-sponge is a **single-capability port**: its one documented job is "soak up stdin & write it atomically to a file". No optional feature leaves are carved beyond the required umbrellas; see [`docs/feature-layout.md`](docs/feature-layout.md) for why.

### Feature matrix

| Feature | Description | Umbrella(s) |
|---|---|---|
| `cli` | All CLI-only dependencies (`clap`, `clap_complete`, `anyhow`, `signal-hook`) and the binary entry point, signal-handler install, mode resolver, and Strict-mode pre-scanner. Library consumers strip via `default-features = false`. | `full`, `sponge-classic`, `sponge-minimal`, `sponge-alias` |
| `sponge-alias` | Installs an additional `sponge` binary alongside `rusty-sponge`. Both share source; argv[0] auto-detect routes `sponge` invocations into Strict mode. | (standalone, implies `cli`) |
| `bench` | Pulls `criterion` and enables `benches/throughput.rs`. Dev-tooling only; outside the convention's leaf surface. Name preserved verbatim from v0.1.x. | (standalone) |

### Preset bundles

| Bundle | Composition | Use case |
|---|---|---|
| `sponge-classic` | `cli` | Drop-in upstream moreutils `sponge` replacement. Strict mode is invoked via `--strict`, `RUSTY_SPONGE_STRICT`, or `sponge-alias` argv[0] auto-detect. |
| `sponge-minimal` | `cli` | Explicit minimal-CLI alias for users who prefer the `<port>-minimal` naming convention seen across other portfolio ports (figlet-minimal, ts-minimal, pwgen-minimal). Identical composition to `sponge-classic`. |

### Keep-list workaround (Cargo features are union-only)

Cargo features cannot subtract from `default`. To get "everything except a specific feature," disable defaults & enumerate the features you want:

```sh
cargo install rusty-sponge --no-default-features --features "cli"
# → bare CLI with no sponge-alias binary, no bench tooling.

cargo install rusty-sponge --no-default-features --features "cli sponge-alias"
# → CLI + the sponge alias binary.
```

For the common cases the named [preset bundles](#preset-bundles) are usually sufficient.

### Library-only consumers

```toml
[dependencies]
rusty-sponge = { version = "0.2", default-features = false }
```

This strips `clap`, `clap_complete`, `anyhow`, & `signal-hook`. The resulting build pulls only `tempfile`, `thiserror`, & the `windows-sys` target-conditional dep (Windows only). The CI `test-no-default` job runs `cargo tree --no-default-features` on every PR & fails the build if any CLI-only dep leaks back in.

### Convention authority

This layout follows the portfolio-wide Cargo Features Convention. The "why" lives in [ADR-0006](https://github.com/jsh562/rustylib/blob/main/specs/adrs/0006-cargo-features-convention-for-portfolio-ports.md); the "what" lives in [`project-instructions.md` §Cargo Feature Surface](https://github.com/jsh562/rustylib/blob/main/project-instructions.md). Every Rusty port from v0.2 onward exposes the same umbrella set (`default` / `full` / `cli` / `<port>-classic`), per-port leaves named in kebab-case, & 2 to 4 preset bundles.

## Compatibility

`rusty-sponge` has two modes:

- **Default mode.** clap-styled flag parser. `--help`, `--version`, the `completions` subcommand, & the `RUSTY_SPONGE_SPILL_MB` env override are all available. Spill threshold defaults to 128 MiB (compile-time constant) so RAM sizing is predictable across hosts.
- **Strict mode** (activated by `--strict`, `RUSTY_SPONGE_STRICT=1`, or invoking the binary as `sponge`). Byte-equal stdout, stderr, exit codes, & the `-h` usage layout against moreutils `sponge` at the pinned upstream commit recorded in [`fixtures/README.md`](fixtures/README.md). `--help`, `--version`, & `completions` MUST be rejected. `RUSTY_SPONGE_SPILL_MB` MUST be ignored.

### Atomic-safety guarantee

When `rusty-sponge` writes to a regular non-symlink file, it writes to a sibling tempfile in the target's parent directory & atomically `rename`s into place. Mid-write failures (SIGKILL, power loss, disk full) leave the original file byte-identical to its pre-invocation state. This is the property the original `sponge` was invented to provide.

The guarantee does NOT apply when:

1. The target is a symlink or non-regular file. The linked file is written through with `O_WRONLY+O_TRUNC`, matching moreutils' `S_ISREG && !S_ISLNK` short-circuit.
2. The cross-volume / shared-handle atomic-rename fallback triggers. Non-atomic copy + truncate-and-rewrite runs as a last resort.

Both fallback paths match moreutils behavior. They are documented limitations, not bugs.

### Documented intentional divergences

1. **`--help` / `--version`**. Default-mode additions; rejected in Strict.
2. **`completions` subcommand**. Default-mode addition; rejected in Strict.
3. **`RUSTY_SPONGE_SPILL_MB` env var**. Honored in Default; ignored in Strict.
4. **Spill threshold default**: 128 MiB compile-time constant vs moreutils' dynamic ½-available-RAM heuristic. Trades RAM-aware sizing for predictability; configurable via env var or library builder.

See [`docs/COMPATIBILITY.md`](docs/COMPATIBILITY.md) for the full per-flag matrix & exit-code table.

## What's not shipped

- **moreutils' dynamic ½-available-RAM spill heuristic.** Replaced with the 128 MiB compile-time constant for predictability across hosts. Users override via `RUSTY_SPONGE_SPILL_MB` or the library `spill_threshold` builder setter.
- **Source-code derivation from moreutils.** This is a clean-room reimplementation. The moreutils `sponge` C source is GPL'd & untouched. Snapshot tests compare runtime output bytes only, which are facts, not creative expression. Same posture as [`uutils/coreutils`](https://github.com/uutils/coreutils).

If you want the original moreutils `sponge`, install it via your platform package manager (`apt install moreutils`, `brew install moreutils`). It coexists fine with this port.

## MSRV

Rust **1.85** (edition 2024). Re-verified against the portfolio's stable-minus-two policy at each release.

## License

Dual-licensed under [MIT](LICENSE) or [Apache-2.0](LICENSE-APACHE) at your option.

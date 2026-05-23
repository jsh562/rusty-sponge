# Fixtures — moreutils `sponge` byte-equality reference

This directory holds the moreutils `sponge` reference fixtures used by the
snapshot tests under `tests/compat_default.rs` and `tests/compat_strict.rs`.
The fixture-capture protocol mirrors the one promoted to the umbrella SAD
during the `rusty-ts` (00001) feature.

## Capture protocol (mandatory)

All fixtures MUST be captured under a pinned environment:

| Variable | Value | Why |
|----------|-------|-----|
| `LC_ALL` | `C.UTF-8` | Locale stability; sponge does not emit timestamps or locale-sensitive output, but pinning prevents future drift if moreutils ever adds locale-dependent error text. |
| moreutils source | `madx/moreutils@<COMMIT_HASH>` — pinned at capture time | `sponge.c` evolves; byte-equal fidelity is meaningless without a pinned reference. |
| Capture platform | WSL2 Ubuntu 24.04 LTS | Matches the Linux CI runner toolchain; `apt install moreutils` provides a vetted distro build of `sponge`. |

The capture script `scripts/capture-sponge.sh` asserts these values at
startup and refuses to run otherwise (HINT-001 lesson: fixtures captured
under wrong env corrupt byte-equality assertions silently).

## Layout

```
fixtures/
├── README.md                  # this file
├── inputs/                    # input bytes piped to moreutils sponge
│   ├── happy_path/
│   ├── existing_target/
│   ├── missing_target/
│   ├── append_existing/
│   ├── append_missing/
│   ├── symlink_target/        # Unix-only
│   └── ...
├── moreutils_outputs/         # captured target-file bytes after sponge runs
│   ├── happy_path/
│   ├── existing_target/
│   ├── missing_target/
│   └── ...
└── scripts/
    └── capture-sponge.sh      # the capture driver
```

## Recapture policy

**Do not regenerate inputs and outputs in the same commit.** Per HINT-001
(promoted from `rusty-ts` iter-5 lessons): if you change an input, the
moreutils output drift is no longer being verified — you're just asserting
that today's run equals today's run. Capture inputs once and freeze them;
regenerate outputs only when the pinned upstream commit advances.

## Pinned upstream version

**moreutils package**: `0.69-1` (Ubuntu 24.04 LTS distro build, captured 2026-05-22 via WSL2 Ubuntu).

The upstream `madx/moreutils` repository tags do not perfectly match Debian package versions; the captured bytes are the source of truth for byte-equality. Re-capture is only triggered when the Ubuntu package advances and the team has decided to bump the pin.

## Captured fixture categories (v0.1.0 baseline)

| Category | Inputs | Outputs | Notes |
|---|---|---|---|
| `happy_path/` | 4 | 4 | empty / small text / binary bytes / 1 MiB ascii |
| `existing_target/` | 2 | 1 | preexisting target + replacement stdin |
| `missing_target/` | 1 | 1 | target created from scratch |
| `append_existing/` | 2 | 1 | `-a` mode prepending existing file contents |
| `append_missing/` | 1 | 1 | `-a` mode against non-existent target |
| `stdout_passthrough/` | 3 | 3 | no file argument → write to stdout |
| `symlink_target/` | 2 | 2 | Unix-only: symlink + linked-file content |
| `strict_h/` | 0 | 3 | `sponge -h` → usage banner to **stdout**, exit 0 |
| `strict_help/` | 0 | 3 | `sponge --help` → getopt error + usage, exit 0 |
| `strict_unknown_option/` | 0 | 9 | `-x`, `-X`, `--some-flag` (per-char getopt iteration) |
| `target_is_directory/` | 0 | 3 | exit 1, stderr `error opening output file: Is a directory` |

## Critical moreutils behaviors discovered at capture time

These shaped the implementation and Strict-mode parity surface:

1. **`sponge` exits 0 for nearly everything**, including `-h`, `--help`, and unknown-option errors. The only non-zero exit observed is when the target is a directory (`exit 1`).
2. **Usage banner goes to stdout, not stderr.** `sponge -h` writes `sponge [-a] <file>: soak up all input from stdin and write it to <file>\n` (72 bytes) to stdout.
3. **`--some-flag` produces per-character getopt errors.** moreutils sponge's POSIX `getopt` strips the leading `--` and iterates through `s`, `o`, `m`, `e`, `-`, `f`, `l`, `g` — emitting `sponge: invalid option -- 'X'` for each (note `a` is missing because `-a` is a valid flag). Total nine error lines for one input. **Strict mode implementation note**: replicating this byte-for-byte in clap is non-trivial; the implementation may need a custom argv pre-parser for Strict mode, or document this as a single-error divergence.
4. **Long flags (`--`) are not recognized at all.** Anything starting with `--` is decomposed character-by-character. There is no concept of long-form options in moreutils sponge.
5. **`error opening output file: <reason>`** is the canonical IO-error format from moreutils sponge. The `<reason>` is the libc `strerror()` of the failed `open(2)` call.

## Recapture policy

**Do not regenerate inputs and outputs in the same commit.** Per HINT-001 (promoted from `rusty-ts` iter-5 lessons): if you change an input, the moreutils output drift is no longer being verified — you're just asserting that today's run equals today's run. Capture inputs once and freeze them; regenerate outputs only when the moreutils package version pin advances.

## Layout

```
fixtures/
├── README.md                       # this file
├── inputs/
│   ├── happy_path/                 # 4 input files (empty / small / binary / large 1 MiB)
│   ├── existing_target/            # .in + .preexisting per case
│   ├── missing_target/             # .in only (target created at runtime)
│   ├── append_existing/            # .in + .preexisting
│   ├── append_missing/             # .in only
│   ├── stdout_passthrough/         # .in only (no file arg)
│   └── symlink_target/             # .in + .linkdest (the linked file's content)
├── moreutils_outputs/
│   ├── happy_path/                 # .target per case
│   ├── existing_target/            # .target per case
│   ├── missing_target/             # .target per case
│   ├── append_existing/            # .target per case
│   ├── append_missing/             # .target per case
│   ├── stdout_passthrough/         # .stdout per case
│   ├── symlink_target/             # symlink + linkdest
│   ├── strict_h/                   # .stdout + .stderr + .exit per case
│   ├── strict_help/                # .stdout + .stderr + .exit per case
│   ├── strict_unknown_option/      # .stdout + .stderr + .exit per case
│   └── target_is_directory/        # .stdout + .stderr + .exit per case
└── scripts/
    └── capture-sponge.sh           # the capture driver (asserts pinned env)
```

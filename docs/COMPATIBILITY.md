# Compatibility Matrix — `rusty-sponge` vs moreutils `sponge`

Reference: moreutils `0.69-1` (Ubuntu 24.04 LTS, captured 2026-05-22 under `LC_ALL=C.UTF-8`). Snapshot fixtures live in `fixtures/moreutils_outputs/`.

## Flag matrix

| Flag / Form              | Default mode                                                                                  | Strict mode                                                                                                            |
|--------------------------|-----------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------|
| `-a`                     | Append to existing target                                                                     | Append (byte-equal moreutils)                                                                                          |
| `--append`               | Long-form alias of `-a` (Rusty extension)                                                     | Rejected — emits `sponge: invalid option -- '-'` to stderr (per STF-003 option A); falls through to normal dispatch    |
| `-h`                     | clap-rendered help (exit 0)                                                                   | **Byte-equal moreutils** — `sponge [-a] <file>: soak up all input from stdin and write it to <file>\n` to STDOUT, exit 0 |
| `--help`                 | clap-rendered help (exit 0)                                                                   | Treated as unknown option, emits one stderr line, exit 0 (option A divergence — moreutils emits 9 per-char errors)     |
| `--version`              | clap-rendered version (exit 0)                                                                | Treated as unknown options (per option A); exit 0                                                                      |
| `--strict`               | Activates Strict mode (Rusty extension)                                                       | Consumed pre-parse (no-op)                                                                                             |
| `--no-strict`            | Explicit Default override; highest precedence                                                 | Consumed pre-parse (no-op)                                                                                             |
| `--spill-mb=N`           | Override in-memory buffer threshold (Rusty extension; default 128 MiB)                        | Rejected as unknown (per option A); spill stays at 128 MiB                                                              |
| `<file>` (positional)    | Atomic in-place replacement via sibling tempfile + rename                                     | Same                                                                                                                   |
| (no file argument)       | Buffered passthrough to stdout                                                                | Same                                                                                                                   |
| Multiple positionals     | Rejected — clap errors, exit 2                                                                | First positional wins; extras silently ignored (matches moreutils `getopt`)                                            |
| `completions <shell>`    | Subcommand: emit shell-completion script for `bash` / `zsh` / `fish` / `powershell`           | Treated as positional file target named "completions" (subcommand NOT honored)                                         |

## Env-var matrix

| Variable                  | Default mode                                                              | Strict mode |
|---------------------------|---------------------------------------------------------------------------|-------------|
| `RUSTY_SPONGE_STRICT=1`   | Activates Strict mode (precedence: `--strict` > env > argv[0]=sponge > Default) | n/a (already in Strict) |
| `RUSTY_SPONGE_SPILL_MB=N` | Override spill threshold; invalid values emit stderr warning + fall back to 128 MiB | Ignored silently |

## Exit-code matrix

| Scenario                                | rusty-sponge Default | rusty-sponge Strict | moreutils sponge |
|-----------------------------------------|----------------------|---------------------|------------------|
| Successful write                        | 0                    | 0                   | 0                |
| `-h`                                    | 0                    | 0                   | 0                |
| `--help` / `--version`                  | 0                    | 0                   | 0 (unknown opt)  |
| Unknown short flag (e.g. `-x`)          | 2 (clap parse error) | 0                   | 0                |
| Unknown long flag (e.g. `--some-flag`)  | 2 (clap parse error) | 0                   | 0                |
| Target is a directory                   | 1                    | 1                   | 1                |
| Multiple positionals                    | 2 (clap)             | 0 (first wins)      | 0 (first wins)   |
| SIGINT / SIGTERM / SIGHUP (Unix)        | 130                  | 130                 | 130 (default OS) |

## Intentional divergences from moreutils

1. **Long-form flag rejection format (STF-003 option A)** — moreutils' POSIX `getopt` iterates byte-by-byte through unknown long-form flags, producing one error line per character. We emit only the first error line. *Why:* implementing per-character iteration would require a custom 30-line argv pre-parser for no documented-user-input benefit. The byte-equal promise is for documented inputs (`-h`, `-x`, target-is-directory) — undocumented inputs (`--garbage`) accept this divergence.
2. **`--help` / `--version`** — moreutils has neither; we add both in Default mode. Strict mode treats them as unknown options.
3. **`completions` subcommand** — moreutils has no shell-completion support; we add the subcommand in Default mode. Strict mode treats `completions` as a positional file target (the literal word "completions"), preserving the moreutils invariant that the first positional is the target.
4. **`RUSTY_SPONGE_SPILL_MB`** — moreutils sponge derives its in-memory threshold dynamically from `RLIMIT` / available RAM. We use a static 128 MiB default with explicit env-var override. *Why:* dynamic-RAM detection is non-trivial cross-platform and the static default works for >99% of real-world inputs.
5. **`--strict` / `--no-strict` / `sponge-alias` feature** — Rusty additions for opt-in compat-mode control. Have no analogue in moreutils.

## Atomic-safety guarantee scope

The atomic-safety guarantee (mid-write failure leaves the target byte-identical to its prior state) applies **only** to the regular-file `rename(2)` path. The following paths explicitly forgo it, matching moreutils behavior:

- **Symlink targets** (FR-010) — opened with `O_WRONLY|O_TRUNC`; a SIGKILL between truncate and write completion leaves the linked file partial.
- **Windows reparse points** — treated as non-regular and follow the write-through path.
- **Cross-volume fallback** (FR-025) — if `rename(2)` cannot proceed atomically (e.g., target on a different filesystem from `$TMPDIR`), Default mode emits `warning: atomic rename unavailable; falling back to non-atomic rewrite of <path>` to stderr and copies through; Strict mode does the same silently.

Refer to the README's "Compatibility statement" section for user-facing wording.

---

**Generation note.** This file is currently hand-authored from the captured fixture set. A future revision may regenerate it from the clap `Cli::command()` introspection per AD-006 (drift-tested via integration test, never via `build.rs`). For v0.1.0 the surface is small enough that hand-maintenance plus the snapshot test suite in `tests/compat_*.rs` provides equivalent drift protection.

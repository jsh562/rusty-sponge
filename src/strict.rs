//! Strict moreutils-compat mode entry point.
//!
//! Bypasses clap entirely (clap can't produce byte-equal moreutils errors)
//! and runs a hand-rolled argv scan that mirrors the documented inputs from
//! the captured moreutils 0.69-1 fixture set:
//!
//! | invocation                | stdout                                                                  | stderr                                  | exit |
//! |---------------------------|-------------------------------------------------------------------------|-----------------------------------------|------|
//! | `sponge -h`               | `sponge [-a] <file>: soak up all input from stdin and write it to <file>` + LF | —                                       | 0    |
//! | `sponge -x` (unknown letter) | —                                                                       | `sponge: invalid option -- 'x'\n`       | 0    |
//! | `sponge file` (success)   | —                                                                       | —                                       | 0    |
//! | `sponge -a file` (append) | —                                                                       | —                                       | 0    |
//! | `sponge dir/` (target=dir)| —                                                                       | `error opening output file: Is a directory\n` | 1    |
//!
//! Per STF-003 (autopilot option A): for any unknown long-form input like
//! `--some-flag`, we emit ONLY the first unknown-option error and continue
//! (one line). moreutils' POSIX `getopt` iterates per-character producing
//! up to N errors; we accept the documented divergence rather than carry a
//! custom getopt-style scanner.

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use crate::{Sponge, SpongeBuilder, Target};

/// The exact usage banner moreutils sponge writes to stdout for `-h` and
/// after some error paths. 72 bytes including trailing newline; captured
/// from moreutils 0.69-1.
const STRICT_USAGE_BANNER: &str =
    "sponge [-a] <file>: soak up all input from stdin and write it to <file>\n";

/// Strict-mode entry. Returns the process exit code.
pub fn run(argv: &[OsString]) -> ExitCode {
    let parsed = parse_argv(argv);

    if parsed.show_usage {
        // moreutils sponge writes its usage banner to STDOUT, not stderr,
        // and exits 0 (STF-005).
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        let _ = out.write_all(STRICT_USAGE_BANNER.as_bytes());
        return ExitCode::SUCCESS;
    }

    // Emit unknown-option errors first. moreutils exits 0 for these — it
    // does NOT bail out. Sponge then falls through to its main path
    // (stdin → stdout or stdin → target).
    if let Some(unk) = parsed.unknown_letters.first() {
        // Per STF-003 option A: emit only the FIRST unknown-option error
        // and then continue (moreutils emits one per char of an unknown
        // long flag; we emit one total).
        let stderr = std::io::stderr();
        let mut err = stderr.lock();
        let _ = writeln!(err, "sponge: invalid option -- '{unk}'");
    }

    // Dispatch.
    let target = match parsed.target {
        Some(path) => Target::File(path.into()),
        None => Target::Stdout,
    };

    // In Strict mode, --help/--version are not recognized (and we already
    // emitted the unknown-option error above). Pre-flight target-is-dir
    // check uses the moreutils-byte-equal error string.
    if let Target::File(ref path) = target {
        if let Ok(meta) = std::fs::symlink_metadata(path) {
            if meta.is_dir() {
                let stderr = std::io::stderr();
                let mut err = stderr.lock();
                let _ = writeln!(err, "error opening output file: Is a directory");
                return ExitCode::from(1);
            }
        }
    }

    // Build the Sponge runtime via the same builder library consumers use,
    // ensuring behavior parity between binary and library paths.
    let builder = SpongeBuilder::new()
        .target(target)
        .append(parsed.append)
        .compat(crate::CompatibilityMode::Strict);
    let mut sponge: Sponge = match builder.build() {
        Ok(s) => s,
        Err(_e) => {
            // Strict mode swallows builder errors as best-effort; this path
            // is exercised only if our own validation rejects a combination
            // moreutils would accept (e.g., -a without target). Emit nothing
            // and exit 0 to match moreutils' "exit 0 for everything but IO"
            // behavior.
            return ExitCode::SUCCESS;
        }
    };

    let stdin = std::io::stdin();
    let locked = stdin.lock();
    match sponge.run(locked) {
        Ok(()) => ExitCode::SUCCESS,
        Err(crate::Error::TargetIsDirectory(_)) => {
            // Edge case if directory check above somehow missed (race).
            let stderr = std::io::stderr();
            let mut err = stderr.lock();
            let _ = writeln!(err, "error opening output file: Is a directory");
            ExitCode::from(1)
        }
        Err(crate::Error::Io(io_err)) => {
            // Moreutils-shaped IO error: `error opening output file: <strerror>`.
            let stderr = std::io::stderr();
            let mut err = stderr.lock();
            let _ = writeln!(err, "error opening output file: {io_err}");
            ExitCode::from(1)
        }
        Err(_) => ExitCode::from(1),
    }
}

/// Result of scanning the strict-mode argv: which flags were seen, which
/// unknown single letters were rejected, the (optional) first positional
/// target. Per moreutils first-wins semantics (STF-003-adjacent), additional
/// positionals are silently dropped.
struct StrictArgs {
    append: bool,
    show_usage: bool,
    unknown_letters: Vec<char>,
    target: Option<OsString>,
}

fn parse_argv(argv: &[OsString]) -> StrictArgs {
    let mut out = StrictArgs {
        append: false,
        show_usage: false,
        unknown_letters: Vec::new(),
        target: None,
    };

    // Skip argv[0] (program name).
    let mut iter = argv.iter().skip(1);
    while let Some(arg) = iter.next() {
        let s = arg.to_string_lossy();

        // Special-case our own --strict / --no-strict — these are consumed
        // by mode resolution upstream and must not reach the strict parser.
        if s == "--strict" || s == "--no-strict" {
            continue;
        }

        // `--` end-of-options sentinel: subsequent args are positionals.
        if s == "--" {
            for rest in iter.by_ref() {
                if out.target.is_none() {
                    out.target = Some(rest.clone());
                }
            }
            break;
        }

        // Short flags (one or more chars after a single `-`).
        if s.starts_with('-') && s.len() >= 2 && !s.starts_with("--") {
            for c in s.chars().skip(1) {
                match c {
                    'a' => out.append = true,
                    'h' => out.show_usage = true,
                    other => {
                        // Per STF-003 option A: record only the FIRST unknown
                        // letter; we emit a single error line in `run()`.
                        if out.unknown_letters.is_empty() {
                            out.unknown_letters.push(other);
                        }
                    }
                }
            }
            continue;
        }

        // Long flags (`--...`). moreutils has no long-form options; treat
        // the leading `--` as an unknown option per STF-003 option A.
        if s.starts_with("--") {
            if out.unknown_letters.is_empty() {
                out.unknown_letters.push('-');
            }
            continue;
        }

        // Positional. First wins per moreutils observed behavior.
        if out.target.is_none() {
            out.target = Some(arg.clone());
        }
    }

    out
}

/// Pre-clap scan for `--strict` / `--no-strict` so the binary can decide
/// whether to enter [`run`] *before* clap gets a chance to print its own
/// help/version messages. Returns `Some(true)` for `--strict`, `Some(false)`
/// for `--no-strict`, `None` otherwise. The last occurrence wins.
pub fn pre_scan_strict_flag(argv: &[OsString]) -> Option<bool> {
    let mut chosen: Option<bool> = None;
    for arg in argv.iter().skip(1) {
        let s = arg.to_string_lossy();
        if s == "--strict" {
            chosen = Some(true);
        } else if s == "--no-strict" {
            chosen = Some(false);
        } else if s == "--" {
            break;
        }
    }
    chosen
}

/// Helper: trim the program name to its basename for path-style argv[0].
#[allow(dead_code)]
fn argv0_basename(argv0: &Path) -> Option<&std::ffi::OsStr> {
    argv0.file_stem()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(parts: &[&str]) -> Vec<OsString> {
        parts.iter().map(|s| OsString::from(*s)).collect()
    }

    #[test]
    fn parse_no_args_means_stdout_target() {
        let p = parse_argv(&argv(&["sponge"]));
        assert!(!p.append);
        assert!(!p.show_usage);
        assert!(p.unknown_letters.is_empty());
        assert!(p.target.is_none());
    }

    #[test]
    fn parse_dash_h_sets_show_usage() {
        let p = parse_argv(&argv(&["sponge", "-h"]));
        assert!(p.show_usage);
    }

    #[test]
    fn parse_dash_a_sets_append() {
        let p = parse_argv(&argv(&["sponge", "-a", "file.txt"]));
        assert!(p.append);
        assert_eq!(p.target.as_deref(), Some(std::ffi::OsStr::new("file.txt")));
    }

    #[test]
    fn parse_dash_x_records_one_unknown_letter() {
        let p = parse_argv(&argv(&["sponge", "-x"]));
        assert_eq!(p.unknown_letters, vec!['x']);
    }

    #[test]
    fn parse_grouped_unknown_records_only_first() {
        // -xyz → option A: only the first unknown letter is captured.
        let p = parse_argv(&argv(&["sponge", "-xyz"]));
        assert_eq!(p.unknown_letters, vec!['x']);
    }

    #[test]
    fn parse_long_unknown_flag_records_dash() {
        // --some-flag → first error is the leading -- per STF-003 option A.
        let p = parse_argv(&argv(&["sponge", "--some-flag"]));
        assert_eq!(p.unknown_letters, vec!['-']);
    }

    #[test]
    fn parse_first_positional_wins() {
        let p = parse_argv(&argv(&["sponge", "first.txt", "second.txt", "third.txt"]));
        assert_eq!(p.target.as_deref(), Some(std::ffi::OsStr::new("first.txt")));
    }

    #[test]
    fn parse_double_dash_consumes_first_positional_only() {
        let p = parse_argv(&argv(&["sponge", "--", "-a", "file.txt"]));
        assert!(!p.append, "-a after -- is a positional, not a flag");
        assert_eq!(p.target.as_deref(), Some(std::ffi::OsStr::new("-a")));
    }

    #[test]
    fn pre_scan_detects_strict() {
        assert_eq!(
            pre_scan_strict_flag(&argv(&["rusty-sponge", "--strict", "out.txt"])),
            Some(true)
        );
    }

    #[test]
    fn pre_scan_detects_no_strict() {
        assert_eq!(
            pre_scan_strict_flag(&argv(&["rusty-sponge", "--no-strict", "out.txt"])),
            Some(false)
        );
    }

    #[test]
    fn pre_scan_returns_none_when_neither() {
        assert_eq!(
            pre_scan_strict_flag(&argv(&["rusty-sponge", "out.txt"])),
            None
        );
    }

    #[test]
    fn pre_scan_last_occurrence_wins() {
        assert_eq!(
            pre_scan_strict_flag(&argv(&["rusty-sponge", "--strict", "--no-strict"])),
            Some(false)
        );
    }
}

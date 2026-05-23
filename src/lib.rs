//! # rusty-sponge
//!
//! A Rust port of the moreutils `sponge` utility: soak up all of stdin and
//! write it atomically to a file. The library is the canonical surface; the
//! CLI binary is a thin wrapper around [`run`].
//!
//! ## Quick start
//!
//! ```no_run
//! use rusty_sponge::{SpongeBuilder, Target, CompatibilityMode};
//! use std::io::Cursor;
//! use std::path::PathBuf;
//!
//! let mut sponge = SpongeBuilder::new()
//!     .target(Target::File(PathBuf::from("output.txt")))
//!     .append(false)
//!     .compat(CompatibilityMode::Default)
//!     .build()?;
//!
//! sponge.run(Cursor::new(b"hello\nworld\n"))?;
//! # Ok::<(), rusty_sponge::Error>(())
//! ```
//!
//! ## Stability (lockstep SemVer)
//!
//! The library and binary share a single crate version. Within the `0.x`
//! series, minor version bumps may introduce breaking changes per standard
//! Cargo semantics. Every public enum and struct is `#[non_exhaustive]` so
//! that variant additions are not breaking changes once `1.0` lands.
//!
//! ## Atomic-safety guarantee
//!
//! When writing to a regular non-symlink file, [`Sponge::run`] writes to a
//! sibling tempfile in the target's parent directory and atomically renames
//! into place. Mid-write failures (panic, IO error, signal) leave the
//! original file untouched. **Symlink targets and the cross-volume fallback
//! path explicitly forgo this guarantee** — see the crate README for the full
//! compatibility statement.

pub mod buffer;
pub mod error;

pub use error::Error;

use std::path::PathBuf;

/// Where the buffered input should be delivered.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Target {
    /// Write to stdout (no file argument case).
    Stdout,
    /// Atomic replacement of the named file. Regular non-symlink targets get
    /// the sibling-tempfile + rename path; symlinks and reparse points fall
    /// through to a non-atomic write-through path.
    File(PathBuf),
}

/// Whether to apply Default-mode ergonomic extensions or Strict moreutils parity.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompatibilityMode {
    /// Default mode: `--help`, `--version`, `completions`, env-var threshold honored.
    #[default]
    Default,
    /// Strict mode: byte-equal moreutils `sponge` for documented inputs;
    /// rejects every Default-mode addition.
    Strict,
}

/// Runtime engine for one sponge invocation. Constructed via [`SpongeBuilder`].
#[non_exhaustive]
#[derive(Debug)]
pub struct Sponge {
    target: Target,
    append: bool,
    spill_threshold: usize,
    /// Held for Phase 7 Strict-mode logic (e.g., FR-025 cross-volume fallback
    /// warning suppression). Not yet read on the MVP path.
    #[allow(dead_code)]
    compat: CompatibilityMode,
}

/// Default spill threshold (128 MiB).
pub const DEFAULT_SPILL_THRESHOLD: usize = 128 * 1024 * 1024;

/// Builder for [`Sponge`]. All chain methods are `#[must_use]`.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct SpongeBuilder {
    target: Target,
    append: bool,
    spill_threshold: usize,
    compat: CompatibilityMode,
}

impl Default for SpongeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SpongeBuilder {
    /// Construct a new builder defaulting to `Target::Stdout`, no-append,
    /// 128 MiB spill threshold, Default mode.
    #[must_use]
    pub fn new() -> Self {
        Self {
            target: Target::Stdout,
            append: false,
            spill_threshold: DEFAULT_SPILL_THRESHOLD,
            compat: CompatibilityMode::Default,
        }
    }

    /// Set the target.
    #[must_use]
    pub fn target(mut self, target: Target) -> Self {
        self.target = target;
        self
    }

    /// Enable `-a` append mode. Reads existing file contents into the buffer
    /// before stdin. Requires `Target::File`; otherwise `build()` returns
    /// [`Error::InvalidBuilderConfiguration`].
    #[must_use]
    pub fn append(mut self, append: bool) -> Self {
        self.append = append;
        self
    }

    /// Set the spill threshold in bytes.
    #[must_use]
    pub fn spill_threshold(mut self, bytes: usize) -> Self {
        self.spill_threshold = bytes;
        self
    }

    /// Set the compatibility mode.
    #[must_use]
    pub fn compat(mut self, compat: CompatibilityMode) -> Self {
        self.compat = compat;
        self
    }

    /// Validate the configuration and build a [`Sponge`].
    pub fn build(self) -> Result<Sponge, Error> {
        // Validation: append requires a file target.
        if self.append && matches!(self.target, Target::Stdout) {
            return Err(Error::InvalidBuilderConfiguration(
                "append requires a file target",
            ));
        }
        // Validation: Strict mode does not honor explicit spill-threshold overrides.
        if self.compat == CompatibilityMode::Strict
            && self.spill_threshold != DEFAULT_SPILL_THRESHOLD
        {
            return Err(Error::CompatibilityViolation(
                "explicit spill threshold not honored in Strict mode",
            ));
        }
        Ok(Sponge {
            target: self.target,
            append: self.append,
            spill_threshold: self.spill_threshold,
            compat: self.compat,
        })
    }
}

impl Sponge {
    /// Drain the reader, write the buffered bytes to the configured target.
    /// On the regular-file path this performs sibling-tempfile + atomic rename;
    /// on the symlink/reparse path the write-through fallback (FR-010) is
    /// pending Polish phase — the MVP returns an error there.
    pub fn run<R: std::io::Read>(&mut self, reader: R) -> Result<(), Error> {
        match &self.target {
            Target::Stdout => {
                // Pipeline-batching mode (US2): drain stdin into the buffer,
                // then emit to stdout in one shot.
                let mut buf = buffer::Buffer::new();
                // For Stdout target, no on-disk spill dir is meaningful — use
                // the system temp dir as a fallback.
                let spill_dir = std::env::temp_dir();
                buf.drain_reader(reader, self.spill_threshold, &spill_dir)?;
                let stdout = std::io::stdout();
                let mut locked = stdout.lock();
                buf.write_to(&mut locked)?;
                Ok(())
            }
            Target::File(path) => {
                validate_target_path(path)?;

                // Spill directory MUST be the target's parent (HINT-002) so
                // that the eventual atomic-rename in `atomic::write_atomic`
                // works without crossing a filesystem boundary.
                let spill_dir = path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| std::path::PathBuf::from("."));

                let mut buf = buffer::Buffer::new();
                buf.drain_reader(reader, self.spill_threshold, &spill_dir)?;

                // FR-010: symlink and reparse-point targets get the
                // non-atomic write-through path; the atomic-safety guarantee
                // does NOT apply on this branch.
                if writethrough::requires_write_through(path) {
                    writethrough::write_through(buf, path, self.append)?;
                } else {
                    atomic::write_atomic(buf, path, self.append)?;
                }
                Ok(())
            }
        }
    }
}

// Internal atomic-write module. Public for integration tests in our own
// `tests/` directory; library consumers should use the [`Sponge`] runtime.
pub mod atomic;

// Non-atomic write-through path for symlink and reparse-point targets (FR-010).
pub mod writethrough;

/// Pre-write validation: reject directory targets (FR-014). Available without
/// the `cli` feature so library consumers can call it.
fn validate_target_path(target: &std::path::Path) -> Result<(), Error> {
    if let Ok(meta) = std::fs::symlink_metadata(target) {
        if meta.is_dir() {
            return Err(Error::TargetIsDirectory(target.to_path_buf()));
        }
    }
    Ok(())
}

// CLI / mode / signal / atomic-write internals are gated behind `cli` because
// they pull clap, signal-hook, and other binary-only deps. Library callers
// configure compat mode via the builder.
#[cfg(feature = "cli")]
pub mod cli;
#[cfg(feature = "cli")]
pub mod mode;
#[cfg(feature = "cli")]
pub mod signal;
#[cfg(feature = "cli")]
pub mod strict;

/// Binary entry-point helper used by both `src/main.rs` and `src/bin/sponge.rs`.
///
/// Library consumers should use [`SpongeBuilder`] directly; this helper exists
/// only to share the binary entry shape between the default `rusty-sponge`
/// binary and the optional `sponge` alias.
#[cfg(feature = "cli")]
pub fn run() -> std::process::ExitCode {
    use clap::Parser;
    use std::process::ExitCode;

    // Install signal handlers as early as possible so that a Ctrl-C / SIGTERM
    // during stdin reading triggers the cancel-flag path and the in-progress
    // tempfile is dropped before exit (FR-011). Errors here are non-fatal.
    if let Err(e) = signal::install_handlers() {
        eprintln!("warning: could not install signal handlers: {e}");
    }

    // Pre-clap Strict-mode detection: if --strict / RUSTY_SPONGE_STRICT=1 /
    // argv[0]=sponge select Strict, dispatch to the byte-equal-moreutils
    // path before clap gets a chance to emit its own help/version text.
    let raw_argv: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let pre_strict = strict::pre_scan_strict_flag(&raw_argv);
    let env_strict = std::env::var_os("RUSTY_SPONGE_STRICT");
    let argv0 = raw_argv.first().cloned();
    let early_mode = mode::resolve(pre_strict, env_strict.as_deref(), argv0.as_deref());
    if early_mode == CompatibilityMode::Strict {
        return strict::run(&raw_argv);
    }

    let cli_args = match cli::Cli::try_parse() {
        Ok(args) => args,
        Err(e) => {
            // clap handles --help / --version / parse-error printing itself.
            e.print().ok();
            // clap returns exit code 0 for --help / --version (the kind ==
            // DisplayHelp/DisplayVersion), non-zero for parse errors.
            return match e.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                    ExitCode::SUCCESS
                }
                _ => ExitCode::from(2),
            };
        }
    };

    // Handle subcommands (currently only `completions`).
    if let Some(cli::Subcommand::Completions { shell }) = cli_args.command {
        use clap::CommandFactory;
        let mut cmd = cli::Cli::command();
        let name = cmd.get_name().to_string();
        clap_complete::generate(shell, &mut cmd, name, &mut std::io::stdout());
        return ExitCode::SUCCESS;
    }

    // Resolve compatibility mode (precedence: --strict > env > argv[0] > Default).
    let argv0 = std::env::args_os().next();
    let env_strict = std::env::var_os("RUSTY_SPONGE_STRICT");
    let compat = mode::resolve(
        cli::strict_flag(&cli_args),
        env_strict.as_deref(),
        argv0.as_deref(),
    );

    // Resolve spill threshold: env var honored only in Default mode.
    let spill_threshold = resolve_spill_threshold(&cli_args, compat);

    // Build the target.
    let target = match cli_args.target {
        Some(path) => Target::File(path),
        None => Target::Stdout,
    };

    // Construct the runtime via the builder so validation goes through the
    // same code path that library consumers use.
    let result = SpongeBuilder::new()
        .target(target)
        .append(cli_args.append)
        .spill_threshold(spill_threshold)
        .compat(compat)
        .build();

    let mut sponge = match result {
        Ok(s) => s,
        Err(e) => {
            eprintln!("rusty-sponge: {e}");
            return ExitCode::from(1);
        }
    };

    let stdin = std::io::stdin();
    let locked = stdin.lock();
    match sponge.run(locked) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Error::Io(io_err)) if io_err.kind() == std::io::ErrorKind::Interrupted => {
            // FR-011: signal-driven cancellation. Conventional Unix exit code
            // for SIGINT is 130 (128 + SIGINT=2). Tempfile cleanup has already
            // happened via Drop in the error path.
            eprintln!("rusty-sponge: cancelled");
            ExitCode::from(130)
        }
        Err(e) => {
            eprintln!("rusty-sponge: {e}");
            ExitCode::from(1)
        }
    }
}

/// Resolve the effective spill threshold from CLI + env, honoring the
/// compatibility-mode rule that Strict mode ignores explicit overrides
/// (FR-016 / FR-017).
#[cfg(feature = "cli")]
fn resolve_spill_threshold(cli_args: &cli::Cli, compat: CompatibilityMode) -> usize {
    if compat == CompatibilityMode::Strict {
        return DEFAULT_SPILL_THRESHOLD;
    }
    let Some(raw) = cli_args.spill_mb.as_deref() else {
        return DEFAULT_SPILL_THRESHOLD;
    };
    match raw.trim().parse::<usize>() {
        Ok(0) | Err(_) => {
            eprintln!(
                "warning: invalid RUSTY_SPONGE_SPILL_MB value '{raw}'; using default 128 MiB"
            );
            DEFAULT_SPILL_THRESHOLD
        }
        Ok(mb) => mb.saturating_mul(1024 * 1024),
    }
}

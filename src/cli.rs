//! Command-line interface for `rusty-sponge`.
//!
//! The parsed `Cli` struct is consumed by `lib.rs::run()`. Mode resolution
//! happens *after* parse (see [`crate::mode::resolve`]) so the precedence
//! ladder can consider the CLI flag, the env var, and `argv[0]` together.

use clap::Parser;
use std::path::{Path, PathBuf};

use crate::Error;

#[derive(Parser, Debug)]
#[command(
    name = "rusty-sponge",
    version,
    about = "Soak up all of stdin and write it atomically to a file.",
    long_about = "A Rust port of moreutils `sponge`. Buffers all of stdin (in memory \
                  up to a configurable threshold, then spills to a tempfile) before \
                  writing the buffered bytes atomically to the target file via a \
                  sibling tempfile + rename. Without a file argument, writes to stdout."
)]
pub struct Cli {
    /// Append to the target instead of replacing it (reads the existing file
    /// first, then concatenates stdin).
    #[arg(short = 'a', long = "append")]
    pub append: bool,

    /// Enable strict moreutils-compat mode. Rejects every Default-mode
    /// extension and emits byte-equal usage/error text vs moreutils sponge.
    #[arg(long, conflicts_with = "no_strict")]
    pub strict: bool,

    /// Explicitly disable strict mode (overrides `RUSTY_SPONGE_STRICT` env
    /// var and `argv[0] = sponge` auto-detect). Highest precedence.
    #[arg(long = "no-strict")]
    pub no_strict: bool,

    /// Override the in-memory spill threshold (Default mode only; ignored
    /// in Strict mode). Default: 128 MiB.
    #[arg(
        long = "spill-mb",
        env = "RUSTY_SPONGE_SPILL_MB",
        hide_env_values = false
    )]
    pub spill_mb: Option<String>,

    /// The file to write to. Omit to write to stdout (pipeline-batching mode).
    pub target: Option<PathBuf>,

    /// Subcommand (currently only `completions`).
    #[command(subcommand)]
    pub command: Option<Subcommand>,
}

#[derive(clap::Subcommand, Debug)]
pub enum Subcommand {
    /// Emit shell completion scripts (Default mode only).
    Completions {
        /// Shell name: bash, zsh, fish, powershell, elvish.
        shell: clap_complete::Shell,
    },
}

/// Resolve the explicit `--strict` / `--no-strict` flag value the user supplied.
/// Returns:
/// - `Some(true)` if `--strict` was given
/// - `Some(false)` if `--no-strict` was given
/// - `None` if neither was given (mode resolution falls through to env / argv[0])
pub fn strict_flag(cli: &Cli) -> Option<bool> {
    if cli.strict {
        Some(true)
    } else if cli.no_strict {
        Some(false)
    } else {
        None
    }
}

/// Pre-write validation: fail fast (before reading any stdin) if the target
/// is an existing directory. Per FR-014 + HINT-002, this MUST run before any
/// expensive IO.
pub fn validate_target(target: &Path) -> Result<(), Error> {
    if let Ok(meta) = std::fs::symlink_metadata(target) {
        if meta.is_dir() {
            return Err(Error::TargetIsDirectory(target.to_path_buf()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_command_factory_compiles() {
        // Smoke test: clap can build the command tree from the derive.
        let cmd = Cli::command();
        assert_eq!(cmd.get_name(), "rusty-sponge");
    }

    #[test]
    fn parse_no_args_means_stdout_target() {
        let cli = Cli::try_parse_from(["rusty-sponge"]).expect("parse should succeed");
        assert!(cli.target.is_none());
        assert!(!cli.append);
        assert!(!cli.strict);
    }

    #[test]
    fn parse_target_file() {
        let cli = Cli::try_parse_from(["rusty-sponge", "out.txt"]).expect("parse should succeed");
        assert_eq!(cli.target, Some(PathBuf::from("out.txt")));
    }

    #[test]
    fn parse_append_flag() {
        let cli =
            Cli::try_parse_from(["rusty-sponge", "-a", "out.txt"]).expect("parse should succeed");
        assert!(cli.append);
    }

    #[test]
    fn parse_long_append_flag() {
        let cli = Cli::try_parse_from(["rusty-sponge", "--append", "out.txt"])
            .expect("parse should succeed");
        assert!(cli.append);
    }

    #[test]
    fn parse_strict_flag() {
        let cli = Cli::try_parse_from(["rusty-sponge", "--strict", "out.txt"])
            .expect("parse should succeed");
        assert!(cli.strict);
        assert_eq!(strict_flag(&cli), Some(true));
    }

    #[test]
    fn parse_no_strict_flag() {
        let cli = Cli::try_parse_from(["rusty-sponge", "--no-strict", "out.txt"])
            .expect("parse should succeed");
        assert!(cli.no_strict);
        assert_eq!(strict_flag(&cli), Some(false));
    }

    #[test]
    fn parse_strict_conflicts_with_no_strict() {
        let result = Cli::try_parse_from(["rusty-sponge", "--strict", "--no-strict", "out.txt"]);
        assert!(result.is_err(), "--strict and --no-strict must conflict");
    }

    #[test]
    fn parse_spill_mb_via_flag() {
        let cli = Cli::try_parse_from(["rusty-sponge", "--spill-mb", "16", "out.txt"])
            .expect("parse should succeed");
        assert_eq!(cli.spill_mb.as_deref(), Some("16"));
    }

    #[test]
    fn validate_target_rejects_directory() {
        let tmpdir = tempfile::tempdir().unwrap();
        let result = validate_target(tmpdir.path());
        assert!(matches!(result, Err(Error::TargetIsDirectory(_))));
    }

    #[test]
    fn validate_target_accepts_nonexistent_file() {
        let tmpdir = tempfile::tempdir().unwrap();
        let nonexistent = tmpdir.path().join("nope.txt");
        assert!(validate_target(&nonexistent).is_ok());
    }

    #[test]
    fn validate_target_accepts_regular_file() {
        let tmpdir = tempfile::tempdir().unwrap();
        let f = tmpdir.path().join("regular.txt");
        std::fs::write(&f, b"hi").unwrap();
        assert!(validate_target(&f).is_ok());
    }
}

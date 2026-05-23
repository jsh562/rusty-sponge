//! Library-level error type for `rusty_sponge`.
//!
//! The library uses `thiserror` to produce typed errors per AD-009; the binary
//! boundary wraps these in `anyhow::Result` for human-readable diagnostics.

use std::path::PathBuf;

/// Errors returned by the `rusty_sponge` library API.
///
/// Marked `#[non_exhaustive]` so future variant additions are not breaking
/// changes within a major version.
#[non_exhaustive]
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("target is a directory: {0}")]
    TargetIsDirectory(PathBuf),

    #[error("invalid builder configuration: {0}")]
    InvalidBuilderConfiguration(&'static str),

    #[error("invalid spill threshold: {0}")]
    SpillThresholdInvalid(String),

    #[error("compatibility violation: {0}")]
    CompatibilityViolation(&'static str),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

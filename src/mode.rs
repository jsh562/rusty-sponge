//! Compatibility mode resolution.
//!
//! Precedence ladder (FR-021, AD-010, HINT-004):
//! 1. Explicit `--strict` flag wins over everything.
//! 2. `RUSTY_SPONGE_STRICT=1` env var (any truthy value).
//! 3. `argv[0]` basename equals `sponge` (after `.exe` strip on Windows).
//! 4. Default mode.

use crate::CompatibilityMode;
use std::ffi::OsStr;
use std::path::Path;

/// Resolve the compatibility mode from CLI flag, env var, and argv[0].
///
/// `strict_flag` is the post-parse value of `--strict`/`--no-strict` if set.
/// `env_strict` is the value of `$RUSTY_SPONGE_STRICT`.
/// `argv0` is the executable name (the first argv element, as the OS provided it).
pub fn resolve(
    strict_flag: Option<bool>,
    env_strict: Option<&OsStr>,
    argv0: Option<&OsStr>,
) -> CompatibilityMode {
    // (1) Explicit flag wins.
    if let Some(flag) = strict_flag {
        return if flag {
            CompatibilityMode::Strict
        } else {
            CompatibilityMode::Default
        };
    }
    // (2) Env var.
    if let Some(value) = env_strict {
        if env_var_is_truthy(value) {
            return CompatibilityMode::Strict;
        }
    }
    // (3) argv[0] basename match (with .exe strip on Windows).
    if let Some(arg0) = argv0 {
        if argv0_implies_strict(arg0) {
            return CompatibilityMode::Strict;
        }
    }
    // (4) Default.
    CompatibilityMode::Default
}

/// Returns true for env-var values commonly meaning "yes" (1, true, yes, on).
/// Case-insensitive; whitespace-trimmed via OsStr conversion.
fn env_var_is_truthy(value: &OsStr) -> bool {
    let Some(s) = value.to_str() else {
        return false;
    };
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Returns true if argv[0]'s basename (with `.exe` suffix stripped on Windows)
/// equals `sponge`.
fn argv0_implies_strict(arg0: &OsStr) -> bool {
    // Use file_stem to strip the extension (.exe on Windows) per HINT-004.
    let Some(stem) = Path::new(arg0).file_stem() else {
        return false;
    };
    stem == OsStr::new("sponge")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_strict_flag_wins() {
        assert_eq!(resolve(Some(true), None, None), CompatibilityMode::Strict);
        assert_eq!(
            resolve(
                Some(false),
                Some(OsStr::new("1")),
                Some(OsStr::new("sponge"))
            ),
            CompatibilityMode::Default,
            "explicit --no-strict beats env and argv[0]"
        );
    }

    #[test]
    fn env_var_truthy_implies_strict() {
        for v in ["1", "true", "yes", "on", "TRUE", " 1 ", "On"] {
            assert_eq!(
                resolve(None, Some(OsStr::new(v)), None),
                CompatibilityMode::Strict,
                "env value {v:?} should imply strict"
            );
        }
    }

    #[test]
    fn env_var_falsy_does_not_imply_strict() {
        for v in ["0", "false", "no", "off", ""] {
            assert_eq!(
                resolve(None, Some(OsStr::new(v)), None),
                CompatibilityMode::Default,
                "env value {v:?} should NOT imply strict"
            );
        }
    }

    #[test]
    fn argv0_sponge_implies_strict() {
        assert_eq!(
            resolve(None, None, Some(OsStr::new("sponge"))),
            CompatibilityMode::Strict
        );
        assert_eq!(
            resolve(None, None, Some(OsStr::new("/usr/local/bin/sponge"))),
            CompatibilityMode::Strict
        );
        // Windows-style .exe suffix without a path. file_stem() strips it on
        // every platform.
        assert_eq!(
            resolve(None, None, Some(OsStr::new("sponge.exe"))),
            CompatibilityMode::Strict,
            "argv[0] = sponge.exe must imply strict per HINT-004"
        );
    }

    #[cfg(windows)]
    #[test]
    fn argv0_sponge_implies_strict_windows_backslash_path() {
        // Windows-only: backslash is the path separator. On Linux/macOS, Path
        // treats `\` as a regular filename character so file_stem() would
        // return `"C:\bin\sponge"` and the comparison would fail.
        assert_eq!(
            resolve(None, None, Some(OsStr::new("C:\\bin\\sponge.exe"))),
            CompatibilityMode::Strict
        );
    }

    #[test]
    fn argv0_rusty_sponge_does_not_imply_strict() {
        assert_eq!(
            resolve(None, None, Some(OsStr::new("rusty-sponge"))),
            CompatibilityMode::Default
        );
        assert_eq!(
            resolve(None, None, Some(OsStr::new("rusty-sponge.exe"))),
            CompatibilityMode::Default
        );
    }

    #[test]
    fn default_when_nothing_set() {
        assert_eq!(resolve(None, None, None), CompatibilityMode::Default);
    }
}

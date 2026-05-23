//! Shared test harness helpers for snapshot and integration tests.
//!
//! Each snapshot test calls [`assert_pinned_env`] at the top of its body to
//! ensure byte-equality comparisons happen under the pinned `LC_ALL=C.UTF-8`
//! environment that captured the fixtures.

#![allow(dead_code)]

use std::ffi::OsStr;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// Assert that the test process is running under the pinned environment used
/// by `fixtures/scripts/capture-sponge.sh`. Bails the test if not.
pub fn assert_pinned_env() {
    let lc_all = std::env::var("LC_ALL").unwrap_or_default();
    assert_eq!(
        lc_all, "C.UTF-8",
        "fixture byte-equality requires LC_ALL=C.UTF-8 (currently '{}'). \
         Set via: LC_ALL=C.UTF-8 cargo test --release",
        lc_all
    );
}

/// Allocate a fresh tempdir scoped to the test. Returned `TempDir` cleans up on drop.
pub fn with_tempdir() -> tempfile::TempDir {
    tempfile::tempdir().expect("could not create test tempdir")
}

/// Spawn a child process and pipe `stdin_bytes` into its stdin. Caller drives
/// the returned `Child` to completion (via `wait_with_output` etc).
pub fn spawn_with_stdin<S, I>(program: S, args: I, stdin_bytes: &[u8]) -> std::io::Result<Child>
where
    S: AsRef<OsStr>,
    I: IntoIterator,
    I::Item: AsRef<OsStr>,
{
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(stdin_bytes)?;
        // Drop closes stdin → child sees EOF.
    }
    Ok(child)
}

/// Resolve a path inside `fixtures/<category>/<name>` relative to the crate root.
pub fn fixture_path(category: &str, name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("fixtures");
    p.push(category);
    p.push(name);
    p
}

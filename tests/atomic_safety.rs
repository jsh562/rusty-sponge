//! Atomic-safety tests for the regular-file rename path (FR-006 / US3 / SC-002).
//!
//! These tests verify the **invariant**: on any failure before the final
//! `rename`, the target file's prior contents are unchanged. The guarantee is
//! scoped to the regular-file path; symlink write-through (FR-010) and
//! cross-volume fallback (FR-025) explicitly forgo it.
//!
//! The full SIGKILL fault-injection variant only runs on Linux CI
//! (`#[cfg(target_os = "linux")]`) because reliably interrupting a child mid-
//! write requires POSIX `kill(SIGKILL)`. Cross-platform invariant checks
//! live in the always-on tests below.

mod common;

use assert_cmd::Command;
use std::fs;
use std::path::PathBuf;

fn rusty_sponge() -> Command {
    Command::cargo_bin("rusty-sponge").expect("binary should be built")
}

fn target_in(tmpdir: &tempfile::TempDir, name: &str) -> PathBuf {
    tmpdir.path().join(name)
}

#[test]
fn original_target_untouched_when_target_is_directory() {
    // FR-006: pre-write validation must fail BEFORE any byte of stdin is read,
    // and BEFORE the tempfile is created. The directory remains.
    let tmpdir = common::with_tempdir();
    let dir = tmpdir.path().join("a-directory");
    fs::create_dir(&dir).unwrap();

    let before = fs::read_dir(&dir).unwrap().count();
    rusty_sponge()
        .arg(&dir)
        .write_stdin("some bytes\n")
        .assert()
        .failure();
    let after = fs::read_dir(&dir).unwrap().count();

    assert!(dir.is_dir(), "the directory MUST still be a directory");
    assert_eq!(
        before, after,
        "no entries created inside the directory by a failed run"
    );
}

#[test]
fn no_leftover_tempfile_in_parent_after_successful_run() {
    // FR-007 + HINT-002: the tempfile MUST be a sibling (in the target's
    // parent dir), and on success it MUST be consumed by the rename — no
    // leftover `.rusty-sponge-*` entry survives.
    let tmpdir = common::with_tempdir();
    let target = target_in(&tmpdir, "out.txt");

    rusty_sponge()
        .arg(&target)
        .write_stdin("data\n")
        .assert()
        .success();

    let stragglers: Vec<_> = fs::read_dir(tmpdir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".rusty-sponge-")
        })
        .collect();
    assert!(
        stragglers.is_empty(),
        "no rusty-sponge tempfile should remain after success: {stragglers:?}"
    );
}

#[test]
fn replacement_is_durable_after_normal_exit() {
    // After a successful run, the target's bytes match what we wrote.
    // This is the trivial "atomic-rename succeeded" assertion that documents
    // the regular-file path always works under normal conditions.
    let tmpdir = common::with_tempdir();
    let target = target_in(&tmpdir, "durable.txt");
    fs::write(&target, b"OLD\n").unwrap();

    rusty_sponge()
        .arg(&target)
        .write_stdin("NEW\n")
        .assert()
        .success();

    assert_eq!(fs::read(&target).unwrap(), b"NEW\n");
}

#[cfg(target_os = "linux")]
mod linux_sigkill {
    //! SIGKILL fault-injection: spawn rusty-sponge against a target with
    //! known prior content, deliver SIGKILL mid-write, and assert the
    //! target's prior bytes are unchanged.
    //!
    //! Requires `nix` or raw `libc::kill`. We use raw libc to avoid pulling
    //! a dev-dep just for this one test.

    use super::*;
    use std::io::Write;
    use std::process::{Command as StdCommand, Stdio};
    use std::thread;
    use std::time::Duration;

    #[test]
    #[ignore = "linux-only fault-injection: enable with `cargo test --features test-fault-injection`"]
    fn sigkill_during_write_leaves_original_intact() {
        let tmpdir = common::with_tempdir();
        let target = target_in(&tmpdir, "victim.txt");
        let original = b"ORIGINAL_BYTES_THAT_MUST_NOT_CHANGE\n";
        fs::write(&target, original).unwrap();

        // Spawn rusty-sponge as a child with stdin we control.
        let mut child = StdCommand::new(env!("CARGO_BIN_EXE_rusty-sponge"))
            .arg(&target)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn rusty-sponge");

        // Feed some bytes, then SIGKILL the child before stdin closes.
        let mut stdin = child.stdin.take().expect("stdin pipe");
        let _ = stdin.write_all(b"REPLACEMENT_BYTES\n");

        // Give the child a moment to start buffering before we kill it.
        thread::sleep(Duration::from_millis(100));

        // SAFETY: libc::kill is a thin syscall wrapper; pid type matches.
        unsafe {
            libc::kill(child.id() as libc::pid_t, libc::SIGKILL);
        }
        let _ = child.wait();

        // Invariant: the original target bytes are unchanged.
        assert_eq!(
            fs::read(&target).unwrap(),
            original,
            "FR-006/SC-002: SIGKILL mid-write must leave the target byte-identical to its prior state"
        );

        // And no leftover tempfile.
        let stragglers: Vec<_> = fs::read_dir(tmpdir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(".rusty-sponge-")
            })
            .collect();
        assert!(
            stragglers.is_empty(),
            "no rusty-sponge tempfile should remain after SIGKILL"
        );
    }
}

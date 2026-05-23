//! `-a` append-mode tests. The atomic-write implementation in
//! `src/atomic.rs::write_atomic` already handles append; these tests assert
//! byte-equality vs the captured moreutils fixtures for the two cases
//! (existing target, missing target).

mod common;

use assert_cmd::Command;
use std::fs;
use std::path::PathBuf;

use common::{assert_pinned_env, fixture_path};

fn rusty_sponge() -> Command {
    Command::cargo_bin("rusty-sponge").expect("binary should be built")
}

fn target_in(tmpdir: &tempfile::TempDir, name: &str) -> PathBuf {
    tmpdir.path().join(name)
}

#[test]
fn append_existing_target_byte_equal() {
    assert_pinned_env();
    let tmpdir = common::with_tempdir();
    let target = target_in(&tmpdir, "log.txt");

    // Pre-populate target with the captured `.preexisting` content.
    let preexisting = fixture_path("inputs", "append_existing/append.preexisting");
    fs::copy(&preexisting, &target).unwrap();

    let input = fs::read(fixture_path("inputs", "append_existing/append.in")).unwrap();
    let expected = fs::read(fixture_path(
        "moreutils_outputs",
        "append_existing/append.target",
    ))
    .unwrap();

    rusty_sponge()
        .arg("-a")
        .arg(&target)
        .write_stdin(input)
        .assert()
        .success();

    assert_eq!(
        fs::read(&target).unwrap(),
        expected,
        "FR-004: -a must concatenate existing + stdin byte-equal to moreutils"
    );
}

#[test]
fn append_missing_target_treats_as_empty() {
    assert_pinned_env();
    let tmpdir = common::with_tempdir();
    let target = target_in(&tmpdir, "fresh.log");
    assert!(!target.exists(), "FR-005 precondition: target absent");

    let input = fs::read(fixture_path("inputs", "append_missing/append.in")).unwrap();
    let expected = fs::read(fixture_path(
        "moreutils_outputs",
        "append_missing/append.target",
    ))
    .unwrap();

    rusty_sponge()
        .arg("-a")
        .arg(&target)
        .write_stdin(input)
        .assert()
        .success();

    assert!(target.exists(), "FR-005: -a with missing target creates it");
    assert_eq!(fs::read(&target).unwrap(), expected);
}

#[test]
fn append_empty_stdin_with_existing_target_preserves_contents() {
    // STF-002 resolution: -a + empty stdin + existing target = no-op rewrite
    // (target byte-content preserved; new inode/mtime acceptable per FR-005).
    let tmpdir = common::with_tempdir();
    let target = target_in(&tmpdir, "preserve.log");
    let original_bytes = b"keep me unchanged\n";
    fs::write(&target, original_bytes).unwrap();

    rusty_sponge()
        .arg("-a")
        .arg(&target)
        .write_stdin("") // empty stdin
        .assert()
        .success();

    assert_eq!(
        fs::read(&target).unwrap(),
        original_bytes,
        "FR-005 STF-002: empty stdin + -a + existing = byte-content preserved"
    );
}

#[test]
fn append_long_form_flag_accepted() {
    // The `--append` long form must also work (per spec/clap).
    let tmpdir = common::with_tempdir();
    let target = target_in(&tmpdir, "long.log");
    fs::write(&target, b"first\n").unwrap();

    rusty_sponge()
        .arg("--append")
        .arg(&target)
        .write_stdin("second\n")
        .assert()
        .success();

    assert_eq!(fs::read(&target).unwrap(), b"first\nsecond\n");
}

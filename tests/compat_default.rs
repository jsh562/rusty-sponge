//! Byte-equal snapshot tests for Default-mode behavior vs captured moreutils
//! `sponge` outputs. Fixtures live under `fixtures/inputs/<category>/*.in` and
//! `fixtures/moreutils_outputs/<category>/*.target`.
//!
//! See `fixtures/README.md` for the capture protocol and pinned moreutils
//! version (0.69-1 Ubuntu 24.04 LTS at v0.1.0 baseline).

mod common;

use assert_cmd::Command;
use std::fs;
use std::path::PathBuf;

use common::{assert_pinned_env, fixture_path};

/// Read the captured-input bytes for a given fixture.
fn read_input(category: &str, name: &str) -> Vec<u8> {
    let p = fixture_path("inputs", &format!("{category}/{name}.in"));
    fs::read(&p).unwrap_or_else(|e| panic!("missing fixture input {}: {e}", p.display()))
}

/// Read the captured-output bytes for a given fixture (the expected target-file
/// content after moreutils sponge ran).
fn read_expected_target(category: &str, name: &str) -> Vec<u8> {
    let p = fixture_path("moreutils_outputs", &format!("{category}/{name}.target"));
    fs::read(&p).unwrap_or_else(|e| panic!("missing fixture output {}: {e}", p.display()))
}

fn rusty_sponge() -> Command {
    Command::cargo_bin("rusty-sponge").expect("binary should be built")
}

fn target_in(tmpdir: &tempfile::TempDir, name: &str) -> PathBuf {
    tmpdir.path().join(name)
}

#[test]
fn happy_path_empty_stdin_produces_zero_byte_target() {
    assert_pinned_env();
    let tmpdir = common::with_tempdir();
    let target = target_in(&tmpdir, "out.txt");
    let input = read_input("happy_path", "empty");
    let expected = read_expected_target("happy_path", "empty");

    rusty_sponge()
        .arg(&target)
        .write_stdin(input)
        .assert()
        .success();

    let actual = fs::read(&target).unwrap();
    assert_eq!(
        actual, expected,
        "byte-equality with moreutils empty-stdin output"
    );
    assert_eq!(actual.len(), 0, "FR-013: empty stdin → zero-byte target");
}

#[test]
fn happy_path_small_text_byte_equal() {
    assert_pinned_env();
    let tmpdir = common::with_tempdir();
    let target = target_in(&tmpdir, "out.txt");
    let input = read_input("happy_path", "small_text");
    let expected = read_expected_target("happy_path", "small_text");

    rusty_sponge()
        .arg(&target)
        .write_stdin(input)
        .assert()
        .success();

    assert_eq!(fs::read(&target).unwrap(), expected);
}

#[test]
fn happy_path_binary_bytes_passthrough() {
    assert_pinned_env();
    let tmpdir = common::with_tempdir();
    let target = target_in(&tmpdir, "out.bin");
    let input = read_input("happy_path", "binary");
    let expected = read_expected_target("happy_path", "binary");

    rusty_sponge()
        .arg(&target)
        .write_stdin(input)
        .assert()
        .success();

    let actual = fs::read(&target).unwrap();
    assert_eq!(
        actual, expected,
        "FR-012: non-UTF-8 bytes must pass through unchanged"
    );
}

#[test]
fn happy_path_large_input_byte_equal() {
    assert_pinned_env();
    let tmpdir = common::with_tempdir();
    let target = target_in(&tmpdir, "out.dat");
    let input = read_input("happy_path", "large");
    let expected = read_expected_target("happy_path", "large");
    assert_eq!(input.len(), 1024 * 1024, "1 MiB fixture by construction");

    rusty_sponge()
        .arg(&target)
        .write_stdin(input)
        .assert()
        .success();

    assert_eq!(fs::read(&target).unwrap(), expected);
}

#[test]
fn existing_target_replacement_byte_equal() {
    assert_pinned_env();
    let tmpdir = common::with_tempdir();
    let target = target_in(&tmpdir, "out.txt");
    // Pre-populate with the captured `.preexisting` content
    let preexisting = fixture_path("inputs", "existing_target/replace.preexisting");
    fs::copy(&preexisting, &target).unwrap();

    let input = read_input("existing_target", "replace");
    let expected = read_expected_target("existing_target", "replace");

    rusty_sponge()
        .arg(&target)
        .write_stdin(input)
        .assert()
        .success();

    assert_eq!(fs::read(&target).unwrap(), expected);
}

#[test]
fn missing_target_creation_byte_equal() {
    assert_pinned_env();
    let tmpdir = common::with_tempdir();
    let target = target_in(&tmpdir, "fresh.txt");
    assert!(!target.exists(), "target must not exist before invocation");

    let input = read_input("missing_target", "created");
    let expected = read_expected_target("missing_target", "created");

    rusty_sponge()
        .arg(&target)
        .write_stdin(input)
        .assert()
        .success();

    assert!(target.exists(), "FR-015: missing target must be created");
    assert_eq!(fs::read(&target).unwrap(), expected);
}

#[test]
fn stdout_passthrough_small_text_byte_equal() {
    // US2: no file argument → buffered passthrough to stdout
    assert_pinned_env();
    let input = read_input("stdout_passthrough", "small");
    let expected = fs::read(fixture_path(
        "moreutils_outputs",
        "stdout_passthrough/small.stdout",
    ))
    .unwrap();

    let output = rusty_sponge()
        .write_stdin(input)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(output, expected);
}

#[test]
fn stdout_passthrough_empty_stdin_emits_nothing() {
    assert_pinned_env();
    let input = read_input("stdout_passthrough", "empty");
    let expected = fs::read(fixture_path(
        "moreutils_outputs",
        "stdout_passthrough/empty.stdout",
    ))
    .unwrap();
    assert_eq!(
        expected.len(),
        0,
        "expected fixture is empty by construction"
    );

    let output = rusty_sponge()
        .write_stdin(input)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(output, expected);
    assert_eq!(output.len(), 0);
}

#[test]
fn stdout_passthrough_binary_bytes_unchanged() {
    assert_pinned_env();
    let input = read_input("stdout_passthrough", "binary");
    let expected = fs::read(fixture_path(
        "moreutils_outputs",
        "stdout_passthrough/binary.stdout",
    ))
    .unwrap();

    let output = rusty_sponge()
        .write_stdin(input)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(
        output, expected,
        "FR-012: non-UTF-8 byte payload passes through stdout unchanged"
    );
}

#[cfg(unix)]
#[test]
fn existing_target_unix_mode_preserved() {
    use std::os::unix::fs::PermissionsExt;
    assert_pinned_env();
    let tmpdir = common::with_tempdir();
    let target = target_in(&tmpdir, "with-mode.txt");
    fs::write(&target, b"OLD\n").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();

    rusty_sponge()
        .arg(&target)
        .write_stdin("NEW\n")
        .assert()
        .success();

    let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "FR-008: prior st_mode preserved on Unix");
}

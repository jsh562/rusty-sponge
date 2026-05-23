//! CLI error-path tests using `assert_cmd`.

mod common;

use assert_cmd::Command;
use predicates::prelude::*;

use common::assert_pinned_env;

fn rusty_sponge() -> Command {
    Command::cargo_bin("rusty-sponge").expect("binary should be built")
}

#[test]
fn target_is_directory_fails_fast_with_clear_error() {
    assert_pinned_env();
    let tmpdir = common::with_tempdir();
    rusty_sponge()
        .arg(tmpdir.path()) // pass the directory as the target
        .write_stdin("anything\n")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("is a directory")
                .or(predicate::str::contains("Is a Directory")),
        );
}

#[test]
fn version_flag_succeeds_in_default_mode() {
    // Default mode exposes --version per FR-018.
    rusty_sponge()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("rusty-sponge"));
}

#[test]
fn help_flag_succeeds_in_default_mode() {
    // Default mode exposes --help per FR-018.
    rusty_sponge()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage"));
}

#[test]
fn spill_mb_env_var_honored_in_default_mode() {
    // FR-016 + FR-017: RUSTY_SPONGE_SPILL_MB tunes the threshold in Default mode.
    // A small valid value runs cleanly (no warning).
    let tmpdir = common::with_tempdir();
    let target = tmpdir.path().join("with_low_threshold.txt");
    rusty_sponge()
        .env("RUSTY_SPONGE_SPILL_MB", "1")
        .arg(&target)
        .write_stdin("hello\n")
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

#[test]
fn spill_mb_env_var_invalid_value_warns_then_uses_default() {
    // FR-017: invalid env value → warn to stderr then continue with default.
    let tmpdir = common::with_tempdir();
    let target = tmpdir.path().join("invalid_threshold.txt");
    rusty_sponge()
        .env("RUSTY_SPONGE_SPILL_MB", "not-a-number")
        .arg(&target)
        .write_stdin("hello\n")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "warning: invalid RUSTY_SPONGE_SPILL_MB",
        ));
}

#[test]
fn spill_mb_env_var_zero_value_warns_then_uses_default() {
    // FR-017: zero is rejected as invalid, falls back to default.
    let tmpdir = common::with_tempdir();
    let target = tmpdir.path().join("zero_threshold.txt");
    rusty_sponge()
        .env("RUSTY_SPONGE_SPILL_MB", "0")
        .arg(&target)
        .write_stdin("hello\n")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "warning: invalid RUSTY_SPONGE_SPILL_MB",
        ));
}

#[test]
fn spill_mb_env_var_ignored_in_strict_mode() {
    // FR-017: Strict mode ignores RUSTY_SPONGE_SPILL_MB entirely (no warning, no override).
    let tmpdir = common::with_tempdir();
    let target = tmpdir.path().join("strict_threshold.txt");
    rusty_sponge()
        .env("RUSTY_SPONGE_SPILL_MB", "not-a-number")
        .arg("--strict")
        .arg(&target)
        .write_stdin("hello\n")
        .assert()
        .success()
        .stderr(predicate::str::is_empty());
}

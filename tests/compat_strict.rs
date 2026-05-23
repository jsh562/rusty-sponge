//! Byte-equal Strict-mode tests vs captured moreutils `sponge` 0.69-1 output.
//!
//! Fixtures live under `fixtures/moreutils_outputs/{strict_h,strict_help,
//! strict_unknown_option,target_is_directory}/`. Each captured case stores
//! three files: `<name>.stdout`, `<name>.stderr`, `<name>.exit`.
//!
//! Per STF-003 (autopilot option A): for long-form unknown flags like
//! `--some-flag`, our Strict mode emits ONLY the first unknown-option
//! error line, where moreutils' POSIX getopt iterates per-character and
//! emits one line per character. Tests for those cases assert against the
//! first line of moreutils' captured stderr, not the full multi-line block.

mod common;

use assert_cmd::Command;
use std::fs;

use common::{assert_pinned_env, fixture_path};

fn rusty_sponge_strict() -> Command {
    let mut cmd = Command::cargo_bin("rusty-sponge").expect("binary built");
    cmd.arg("--strict");
    cmd
}

fn read_fixture_bytes(category: &str, name: &str, kind: &str) -> Vec<u8> {
    let p = fixture_path("moreutils_outputs", &format!("{category}/{name}.{kind}"));
    fs::read(&p).unwrap_or_else(|e| panic!("missing fixture {}: {e}", p.display()))
}

fn read_fixture_exit(category: &str, name: &str) -> i32 {
    let raw = String::from_utf8(read_fixture_bytes(category, name, "exit"))
        .expect("exit fixture is ASCII");
    raw.trim().parse().expect("exit fixture parses as integer")
}

#[test]
fn strict_dash_h_emits_usage_to_stdout() {
    assert_pinned_env();
    let expected_stdout = read_fixture_bytes("strict_h", "dash_h", "stdout");
    let expected_exit = read_fixture_exit("strict_h", "dash_h");

    let output = rusty_sponge_strict()
        .arg("-h")
        .write_stdin("")
        .assert()
        .get_output()
        .clone();

    assert_eq!(
        output.stdout, expected_stdout,
        "STF-005: -h usage banner is byte-equal moreutils and goes to STDOUT"
    );
    assert_eq!(
        output.status.code().unwrap_or(-1),
        expected_exit,
        "moreutils sponge -h exits 0"
    );
}

#[test]
fn strict_dash_x_emits_unknown_option_to_stderr() {
    assert_pinned_env();
    let expected_stderr = read_fixture_bytes("strict_unknown_option", "dash_x", "stderr");
    let expected_exit = read_fixture_exit("strict_unknown_option", "dash_x");

    let output = rusty_sponge_strict()
        .arg("-x")
        .write_stdin("")
        .assert()
        .get_output()
        .clone();

    assert_eq!(
        output.stderr, expected_stderr,
        "byte-equal moreutils sponge: invalid option -- 'x'"
    );
    assert_eq!(output.status.code().unwrap_or(-1), expected_exit);
}

#[test]
fn strict_dash_cap_x_emits_unknown_option_to_stderr() {
    assert_pinned_env();
    let expected_stderr = read_fixture_bytes("strict_unknown_option", "dash_capX", "stderr");

    let output = rusty_sponge_strict()
        .arg("-X")
        .write_stdin("")
        .assert()
        .get_output()
        .clone();

    assert_eq!(output.stderr, expected_stderr);
}

#[test]
fn strict_long_unknown_emits_first_error_only_per_option_a() {
    // moreutils captured: nine per-character errors. STF-003 option A: we
    // emit ONLY the FIRST one (`sponge: invalid option -- '-'`).
    assert_pinned_env();
    let moreutils_full = read_fixture_bytes("strict_unknown_option", "long_unknown", "stderr");
    let first_line: Vec<u8> = moreutils_full
        .split(|&b| b == b'\n')
        .next()
        .unwrap()
        .iter()
        .copied()
        .chain(std::iter::once(b'\n'))
        .collect();

    let output = rusty_sponge_strict()
        .arg("--some-flag")
        .write_stdin("")
        .assert()
        .get_output()
        .clone();

    assert_eq!(
        output.stderr, first_line,
        "STF-003 option A: only the first per-character error is emitted; \
         documented divergence from moreutils' 9-error output for undocumented inputs"
    );
}

#[test]
fn strict_target_is_directory_byte_equal_error_exit_1() {
    assert_pinned_env();
    let expected_stderr = read_fixture_bytes("target_is_directory", "dir", "stderr");
    let expected_exit = read_fixture_exit("target_is_directory", "dir");

    let tmpdir = common::with_tempdir();
    let output = rusty_sponge_strict()
        .arg(tmpdir.path())
        .write_stdin("")
        .assert()
        .get_output()
        .clone();

    assert_eq!(
        output.stderr, expected_stderr,
        "STF-004: error opening output file: Is a directory"
    );
    assert_eq!(
        output.status.code().unwrap_or(-1),
        expected_exit,
        "moreutils sponge target-is-directory exits 1 (only non-zero case)"
    );
}

#[test]
fn strict_env_var_activates_strict_mode() {
    // FR-021: RUSTY_SPONGE_STRICT=1 activates Strict even without --strict flag.
    let mut cmd = Command::cargo_bin("rusty-sponge").expect("binary built");
    cmd.env("RUSTY_SPONGE_STRICT", "1");
    cmd.arg("-h");

    let output = cmd.write_stdin("").assert().success().get_output().clone();

    let expected = read_fixture_bytes("strict_h", "dash_h", "stdout");
    assert_eq!(output.stdout, expected, "FR-021: env activates Strict");
}

#[test]
fn strict_normal_run_with_file_target_succeeds() {
    // Sanity: --strict with a normal file target works just like default.
    let tmpdir = common::with_tempdir();
    let target = tmpdir.path().join("strict_ok.txt");

    rusty_sponge_strict()
        .arg(&target)
        .write_stdin("hello strict\n")
        .assert()
        .success();

    assert_eq!(fs::read(&target).unwrap(), b"hello strict\n");
}

#[test]
fn strict_extra_positionals_first_wins() {
    // FR-024 (corrected via Clarification Q2): first positional wins;
    // additional positionals silently ignored.
    let tmpdir = common::with_tempdir();
    let first = tmpdir.path().join("first.txt");
    let second = tmpdir.path().join("second.txt");

    rusty_sponge_strict()
        .arg(&first)
        .arg(&second)
        .write_stdin("data\n")
        .assert()
        .success();

    assert!(first.exists(), "first positional must receive the data");
    assert!(
        !second.exists(),
        "second positional must be silently ignored"
    );
    assert_eq!(fs::read(&first).unwrap(), b"data\n");
}

#[test]
fn strict_dash_a_with_existing_target() {
    // FR-004 in Strict mode: -a still works.
    let tmpdir = common::with_tempdir();
    let target = tmpdir.path().join("log.txt");
    fs::write(&target, b"first\n").unwrap();

    rusty_sponge_strict()
        .arg("-a")
        .arg(&target)
        .write_stdin("second\n")
        .assert()
        .success();

    assert_eq!(fs::read(&target).unwrap(), b"first\nsecond\n");
}

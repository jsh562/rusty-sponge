//! FR-010 integration tests: symlink target write-through (Unix-only).
//!
//! Validates that when the CLI target is a symbolic link, rusty-sponge writes
//! THROUGH the link (updates the linked file's bytes) rather than replacing
//! the symlink itself with a fresh regular file.

#![cfg(unix)]

mod common;

use assert_cmd::Command;
use std::fs;

fn rusty_sponge() -> Command {
    Command::cargo_bin("rusty-sponge").expect("binary built")
}

#[test]
fn symlink_target_writes_through_to_linked_file() {
    let tmpdir = common::with_tempdir();
    let realfile = tmpdir.path().join("real.txt");
    let link = tmpdir.path().join("via.link");

    fs::write(&realfile, b"original\n").unwrap();
    std::os::unix::fs::symlink(&realfile, &link).unwrap();

    rusty_sponge()
        .arg(&link)
        .write_stdin("via-link\n")
        .assert()
        .success();

    // Linked file's bytes were replaced.
    assert_eq!(fs::read(&realfile).unwrap(), b"via-link\n");
    // The symlink itself is preserved (not turned into a regular file).
    let link_meta = fs::symlink_metadata(&link).unwrap();
    assert!(
        link_meta.file_type().is_symlink(),
        "FR-010: symlink target must remain a symlink after write-through"
    );
    // The link still resolves to realfile.
    let resolved = fs::read_link(&link).unwrap();
    assert_eq!(resolved, realfile);
}

#[test]
fn symlink_target_append_concatenates_to_linked_file() {
    let tmpdir = common::with_tempdir();
    let realfile = tmpdir.path().join("real-append.txt");
    let link = tmpdir.path().join("via-append.link");

    fs::write(&realfile, b"first\n").unwrap();
    std::os::unix::fs::symlink(&realfile, &link).unwrap();

    rusty_sponge()
        .arg("-a")
        .arg(&link)
        .write_stdin("second\n")
        .assert()
        .success();

    assert_eq!(fs::read(&realfile).unwrap(), b"first\nsecond\n");
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn symlink_target_byte_equal_to_captured_moreutils_output() {
    // Use the captured symlink fixture from iter-1's capture session.
    let input = fs::read(common::fixture_path(
        "inputs",
        "symlink_target/via_symlink.in",
    ))
    .unwrap();
    let linkdest_initial = fs::read(common::fixture_path(
        "inputs",
        "symlink_target/via_symlink.linkdest",
    ))
    .unwrap();
    let expected_linkdest = fs::read(common::fixture_path(
        "moreutils_outputs",
        "symlink_target/via_symlink.linkdest",
    ))
    .unwrap();

    let tmpdir = common::with_tempdir();
    let realfile = tmpdir.path().join("real.bin");
    let link = tmpdir.path().join("via.symlink");
    fs::write(&realfile, &linkdest_initial).unwrap();
    std::os::unix::fs::symlink(&realfile, &link).unwrap();

    rusty_sponge()
        .arg(&link)
        .write_stdin(input)
        .assert()
        .success();

    assert_eq!(
        fs::read(&realfile).unwrap(),
        expected_linkdest,
        "FR-010: byte-equal to moreutils symlink write-through"
    );
}

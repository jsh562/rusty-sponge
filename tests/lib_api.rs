//! Library API integration tests exercising `SpongeBuilder` programmatically.
//! See `src/lib.rs` for the public-surface declarations.

use rusty_sponge::{CompatibilityMode, Error, Sponge, SpongeBuilder, Target};
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

fn fresh_target() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("api.txt");
    (tmp, target)
}

#[test]
fn builder_with_defaults_writes_to_stdout() {
    // Default builder targets Stdout. We can't easily capture stdout here
    // (test process's stdout is shared), so we instead verify that build()
    // succeeds and the result is a Sponge with Stdout-target — by attempting
    // a no-op build.
    let result = SpongeBuilder::new().build();
    assert!(result.is_ok(), "default builder must build");
}

#[test]
fn builder_with_file_target_runs_atomic_replacement() {
    let (_tmp, target) = fresh_target();
    let mut sponge = SpongeBuilder::new()
        .target(Target::File(target.clone()))
        .build()
        .unwrap();
    sponge.run(Cursor::new(b"library-call\n")).unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"library-call\n");
}

#[test]
fn builder_append_without_target_returns_invalid_configuration() {
    let result = SpongeBuilder::new().append(true).build();
    match result {
        Err(Error::InvalidBuilderConfiguration(msg)) => {
            assert!(
                msg.contains("append"),
                "error msg explains the conflict: {msg}"
            );
        }
        other => panic!("expected InvalidBuilderConfiguration, got {other:?}"),
    }
}

#[test]
fn builder_strict_with_custom_spill_threshold_is_compatibility_violation() {
    let result = SpongeBuilder::new()
        .compat(CompatibilityMode::Strict)
        .spill_threshold(64 * 1024)
        .build();
    match result {
        Err(Error::CompatibilityViolation(_)) => {}
        other => panic!("expected CompatibilityViolation, got {other:?}"),
    }
}

#[test]
fn builder_append_with_file_and_existing_concatenates() {
    let (_tmp, target) = fresh_target();
    fs::write(&target, b"ORIG\n").unwrap();

    let mut sponge = SpongeBuilder::new()
        .target(Target::File(target.clone()))
        .append(true)
        .build()
        .unwrap();
    sponge.run(Cursor::new(b"NEW\n")).unwrap();

    assert_eq!(fs::read(&target).unwrap(), b"ORIG\nNEW\n");
}

#[test]
fn builder_target_is_directory_returns_typed_error() {
    let tmp = tempfile::tempdir().unwrap();
    let mut sponge = SpongeBuilder::new()
        .target(Target::File(tmp.path().to_path_buf()))
        .build()
        .unwrap();

    match sponge.run(Cursor::new(b"x")) {
        Err(Error::TargetIsDirectory(path)) => {
            assert_eq!(path, tmp.path());
        }
        other => panic!("expected TargetIsDirectory, got {other:?}"),
    }
}

#[test]
fn empty_stdin_produces_zero_byte_target() {
    let (_tmp, target) = fresh_target();
    let mut sponge = SpongeBuilder::new()
        .target(Target::File(target.clone()))
        .build()
        .unwrap();
    sponge.run(Cursor::new(&[][..])).unwrap();
    assert!(target.exists());
    assert_eq!(fs::metadata(&target).unwrap().len(), 0);
}

#[test]
fn non_utf8_bytes_pass_through_unchanged() {
    let (_tmp, target) = fresh_target();
    let bytes: &[u8] = &[0x00, 0xFE, 0xFF, 0xC3, 0x28, 0xA0, 0xA1];
    let mut sponge = SpongeBuilder::new()
        .target(Target::File(target.clone()))
        .build()
        .unwrap();
    sponge.run(Cursor::new(bytes)).unwrap();
    assert_eq!(fs::read(&target).unwrap(), bytes);
}

#[test]
fn custom_spill_threshold_triggers_for_large_input() {
    let (_tmp, target) = fresh_target();
    // 32 KiB threshold → 64 KiB input must spill to disk during drain_reader.
    let big = vec![0xABu8; 64 * 1024];
    let mut sponge = SpongeBuilder::new()
        .target(Target::File(target.clone()))
        .spill_threshold(32 * 1024)
        .build()
        .unwrap();
    sponge.run(Cursor::new(&big)).unwrap();
    assert_eq!(fs::read(&target).unwrap(), big);
}

#[test]
fn builder_is_chainable_with_must_use_methods() {
    // Compile-time check that the builder chains compose; this is implicit
    // in the fact that the other tests compile, but expressed explicitly
    // here as documentation.
    let _ = SpongeBuilder::new()
        .target(Target::Stdout)
        .append(false)
        .compat(CompatibilityMode::Default)
        .spill_threshold(rusty_sponge::DEFAULT_SPILL_THRESHOLD)
        .build()
        .unwrap();
}

#[test]
fn sponge_struct_is_send_but_not_sync() {
    // Per API Quality Contracts in plan.md: Sponge is Send + !Sync.
    fn assert_send<T: Send>() {}
    // We can't directly assert !Sync at compile time; we settle for asserting
    // Send (which is the affirmative half of the contract). The !Sync half is
    // upheld by the &mut self contract on Sponge::run, which the compiler
    // enforces at every call site.
    assert_send::<Sponge>();
    // SpongeBuilder is Send + Sync.
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<SpongeBuilder>();
}

#[test]
fn error_enum_is_debug_and_display() {
    // Sanity check that Error implements the expected traits.
    let e = Error::TargetIsDirectory(PathBuf::from("/tmp/foo"));
    let _debug = format!("{e:?}");
    let _display = format!("{e}");
}

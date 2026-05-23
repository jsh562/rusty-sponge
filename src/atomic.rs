//! Atomic in-place file rewrite: sibling tempfile + atomic rename.
//!
//! Per AD-001/AD-014: write to a sibling tempfile in `target.parent()`, then
//! call `NamedTempFile::persist(target)` (which wraps `std::fs::rename`, with
//! Windows `FileRenameInfoEx`+POSIX semantics where available). Mid-write
//! failures leave the original target byte-identical to its prior state
//! (FR-006).
//!
//! Mode preservation on Unix (FR-008): if the target exists as a regular
//! non-symlink file, capture its `st_mode` and reapply to the tempfile before
//! persist. Read-only attribute preservation on Windows (FR-009): same idea
//! for `Permissions::readonly()`.
//!
//! Append mode (`-a`, FR-004): if requested AND the target exists, copy its
//! current bytes to the tempfile first, *then* write the incoming buffer.
//! Missing target with `-a` is a no-op per FR-005.

use std::fs;
use std::io;
use std::path::Path;

use tempfile::NamedTempFile;

use crate::{Error, buffer::Buffer};

/// Write `buffer` to `target` atomically. See module docs.
pub fn write_atomic(buffer: Buffer, target: &Path, append: bool) -> Result<(), Error> {
    // Resolve the sibling-tempfile directory. Use target's parent or "." as a
    // last-resort default — `tempfile::Builder::tempfile_in(".")` is well-defined.
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));

    let mut tempfile: NamedTempFile = tempfile::Builder::new()
        .prefix(".rusty-sponge-")
        .tempfile_in(parent)?;

    // -a append: pre-copy existing target bytes into the tempfile, BEFORE we
    // dump the incoming buffer. If the target is missing, this is silently a
    // no-op (FR-005).
    if append {
        if let Ok(mut existing) = fs::File::open(target) {
            io::copy(&mut existing, tempfile.as_file_mut())?;
        }
    }

    // Write the buffered stdin bytes. Empty buffer → zero bytes written
    // (FR-013); tempfile remains a valid 0-byte file and rename still happens.
    buffer.write_to(tempfile.as_file_mut())?;

    // Best-effort durability — match moreutils sponge (which does NOT call
    // fsync) but at least sync our own writes through to the OS buffer cache.
    // We do not fsync the parent directory; this matches moreutils.
    tempfile.as_file_mut().sync_data().ok();

    // Preserve mode/attrs from the existing target, if one exists.
    if let Ok(existing_meta) = fs::symlink_metadata(target) {
        if existing_meta.file_type().is_file() {
            preserve_perms_from(&existing_meta, tempfile.path())?;
        }
    }

    // Atomic rename. `NamedTempFile::persist()` wraps `std::fs::rename`, which
    // employs Windows POSIX-semantics where available (Rust 1.81+).
    tempfile.persist(target).map_err(|e| Error::Io(e.error))?;
    Ok(())
}

#[cfg(unix)]
fn preserve_perms_from(meta: &fs::Metadata, tempfile_path: &Path) -> Result<(), Error> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let mode = meta.mode();
    let mut perms = fs::metadata(tempfile_path)?.permissions();
    perms.set_mode(mode);
    fs::set_permissions(tempfile_path, perms)?;
    Ok(())
}

#[cfg(windows)]
fn preserve_perms_from(meta: &fs::Metadata, tempfile_path: &Path) -> Result<(), Error> {
    let was_readonly = meta.permissions().readonly();
    let mut perms = fs::metadata(tempfile_path)?.permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(was_readonly);
    fs::set_permissions(tempfile_path, perms)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::path::PathBuf;

    fn empty_buffer() -> Buffer {
        Buffer::new()
    }

    fn buffer_from(bytes: &[u8]) -> Buffer {
        let mut b = Buffer::new();
        let tmpdir = tempfile::tempdir().unwrap();
        b.drain_reader(Cursor::new(bytes), 1 << 30, tmpdir.path())
            .unwrap();
        // Note: tmpdir drops here but the InMemory variant doesn't reference it.
        b
    }

    fn target_in(tmpdir: &Path, name: &str) -> PathBuf {
        tmpdir.join(name)
    }

    #[test]
    fn writes_buffer_atomically_to_new_target() {
        let tmpdir = tempfile::tempdir().unwrap();
        let target = target_in(tmpdir.path(), "out.txt");
        write_atomic(buffer_from(b"hello\n"), &target, false).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"hello\n");
    }

    #[test]
    fn empty_buffer_creates_zero_byte_target() {
        let tmpdir = tempfile::tempdir().unwrap();
        let target = target_in(tmpdir.path(), "empty.txt");
        write_atomic(empty_buffer(), &target, false).unwrap();
        assert!(target.exists());
        assert_eq!(fs::metadata(&target).unwrap().len(), 0);
    }

    #[test]
    fn binary_bytes_passthrough_unchanged() {
        let tmpdir = tempfile::tempdir().unwrap();
        let target = target_in(tmpdir.path(), "bin.dat");
        let bytes: &[u8] = &[0x00, 0xFE, 0xFF, 0xC3, 0x28, 0xA0, 0xA1];
        write_atomic(buffer_from(bytes), &target, false).unwrap();
        assert_eq!(fs::read(&target).unwrap(), bytes);
    }

    #[test]
    fn replaces_existing_target() {
        let tmpdir = tempfile::tempdir().unwrap();
        let target = target_in(tmpdir.path(), "replace.txt");
        fs::write(&target, b"OLD\n").unwrap();
        write_atomic(buffer_from(b"NEW\n"), &target, false).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"NEW\n");
    }

    #[test]
    fn append_mode_concatenates_existing_and_stdin() {
        let tmpdir = tempfile::tempdir().unwrap();
        let target = target_in(tmpdir.path(), "append.txt");
        fs::write(&target, b"original\n").unwrap();
        write_atomic(buffer_from(b"appended\n"), &target, true).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"original\nappended\n");
    }

    #[test]
    fn append_mode_missing_target_treats_as_empty() {
        let tmpdir = tempfile::tempdir().unwrap();
        let target = target_in(tmpdir.path(), "missing.txt");
        // No pre-existing target.
        write_atomic(buffer_from(b"first\n"), &target, true).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"first\n");
    }

    #[test]
    fn append_mode_empty_stdin_preserves_existing() {
        let tmpdir = tempfile::tempdir().unwrap();
        let target = target_in(tmpdir.path(), "preserve.txt");
        fs::write(&target, b"keep me\n").unwrap();
        write_atomic(empty_buffer(), &target, true).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"keep me\n");
    }

    #[cfg(unix)]
    #[test]
    fn unix_mode_bits_preserved_on_replacement() {
        use std::os::unix::fs::PermissionsExt;
        let tmpdir = tempfile::tempdir().unwrap();
        let target = target_in(tmpdir.path(), "perms.txt");
        fs::write(&target, b"old\n").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
        write_atomic(buffer_from(b"new\n"), &target, false).unwrap();
        let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o640,
            "prior mode must be preserved on atomic replace"
        );
    }
}

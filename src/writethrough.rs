//! Non-atomic write-through path for symlink and reparse-point targets (FR-010).
//!
//! moreutils `sponge` uses `fopen(path, "w")` which follows symbolic links and
//! truncates the underlying file. POSIX `rename(2)` would replace the symlink
//! itself with a fresh regular file — diverging from moreutils. We therefore
//! detect symlink targets up the call chain (in [`crate::Sponge::run`]) and
//! dispatch here, where we use `OpenOptions` with `truncate(true)` (or
//! `append(true)` for `-a` mode) so the linked file is updated and the link
//! itself stays in place.
//!
//! ## Atomic-safety scope
//!
//! Per FR-006: the atomic-safety guarantee does **NOT** apply on this path.
//! `OpenOptions::truncate(true).open(...)` zeroes the linked file BEFORE we
//! write the buffer; a mid-write failure can leave the linked file partial.
//! This matches moreutils behavior and is documented in the compatibility
//! statement. Use [`crate::atomic::write_atomic`] for the regular-file path
//! when you need the atomic-safety guarantee.

use std::fs::OpenOptions;
use std::path::Path;

use crate::{Error, buffer::Buffer};

/// Write `buffer` to the target by following any symlink and truncating (or
/// appending to) the linked file. Non-atomic — see module docs.
pub fn write_through(buffer: Buffer, target: &Path, append: bool) -> Result<(), Error> {
    let mut file = if append {
        // Append mode: open existing linked file for append; create if missing
        // (matches moreutils which would create-on-missing in append mode).
        // O_APPEND semantics: writes go to current EOF atomically per-syscall,
        // which is equivalent to "read existing + append stdin + truncate-write"
        // in end-state (the moreutils approach).
        OpenOptions::new().create(true).append(true).open(target)?
    } else {
        // Non-append: truncate the linked file and write buffer.
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(target)?
    };

    buffer.write_to(&mut file)?;
    // Best-effort durability; matches moreutils (no fsync there either).
    file.sync_data().ok();
    Ok(())
}

/// Decide whether `target` requires the write-through path. Returns true for
/// POSIX symlinks (`is_symlink()`) and for any non-regular existing file
/// (FIFOs, devices, Windows reparse points). Missing targets return false —
/// the caller dispatches to the atomic-rename path, which creates them.
pub fn requires_write_through(target: &Path) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(target) else {
        return false;
    };
    let ft = meta.file_type();
    if ft.is_symlink() {
        return true;
    }
    // is_file() and is_dir() are the two "regular" kinds. Anything else
    // (FIFOs, char devices, sockets, Windows reparse points that aren't
    // junctions/dirs) should not get the atomic-rename path.
    if !ft.is_file() && !ft.is_dir() {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn buffer_from(bytes: &[u8]) -> Buffer {
        let mut b = Buffer::new();
        let tmpdir = tempfile::tempdir().unwrap();
        b.drain_reader(Cursor::new(bytes), 1 << 30, tmpdir.path())
            .unwrap();
        b
    }

    #[test]
    fn write_through_to_new_file_creates_with_content() {
        let tmpdir = tempfile::tempdir().unwrap();
        let target = tmpdir.path().join("new.txt");
        write_through(buffer_from(b"hello\n"), &target, false).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"hello\n");
    }

    #[test]
    fn write_through_truncates_existing_file() {
        let tmpdir = tempfile::tempdir().unwrap();
        let target = tmpdir.path().join("preexisting.txt");
        std::fs::write(&target, b"OLD_CONTENT\n").unwrap();
        write_through(buffer_from(b"NEW\n"), &target, false).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"NEW\n");
    }

    #[test]
    fn write_through_append_mode_concatenates() {
        let tmpdir = tempfile::tempdir().unwrap();
        let target = tmpdir.path().join("append.txt");
        std::fs::write(&target, b"first\n").unwrap();
        write_through(buffer_from(b"second\n"), &target, true).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"first\nsecond\n");
    }

    #[test]
    fn requires_write_through_false_for_missing_target() {
        let tmpdir = tempfile::tempdir().unwrap();
        let missing = tmpdir.path().join("does-not-exist");
        assert!(!requires_write_through(&missing));
    }

    #[test]
    fn requires_write_through_false_for_regular_file() {
        let tmpdir = tempfile::tempdir().unwrap();
        let f = tmpdir.path().join("regular.txt");
        std::fs::write(&f, b"x").unwrap();
        assert!(!requires_write_through(&f));
    }

    #[cfg(unix)]
    #[test]
    fn requires_write_through_true_for_unix_symlink() {
        let tmpdir = tempfile::tempdir().unwrap();
        let realfile = tmpdir.path().join("real.txt");
        std::fs::write(&realfile, b"linked\n").unwrap();
        let link = tmpdir.path().join("via.link");
        std::os::unix::fs::symlink(&realfile, &link).unwrap();
        assert!(requires_write_through(&link));
    }

    #[cfg(unix)]
    #[test]
    fn write_through_unix_symlink_updates_linked_file_keeps_link() {
        let tmpdir = tempfile::tempdir().unwrap();
        let realfile = tmpdir.path().join("real.txt");
        std::fs::write(&realfile, b"original\n").unwrap();
        let link = tmpdir.path().join("via.link");
        std::os::unix::fs::symlink(&realfile, &link).unwrap();

        write_through(buffer_from(b"via the link\n"), &link, false).unwrap();

        // The linked file's bytes are replaced.
        assert_eq!(std::fs::read(&realfile).unwrap(), b"via the link\n");
        // The link itself is still a symbolic link pointing at realfile.
        let link_meta = std::fs::symlink_metadata(&link).unwrap();
        assert!(
            link_meta.file_type().is_symlink(),
            "FR-010: the symlink itself MUST be preserved (not replaced)"
        );
    }
}

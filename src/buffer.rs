//! Hybrid in-memory + tempfile-spill buffer for soaking up stdin.
//!
//! Per AD-002: small inputs stay in a `Vec<u8>`; once the buffer would
//! exceed the spill threshold, the in-memory bytes are flushed into a
//! sibling tempfile and the remainder of stdin streams through a
//! `BufWriter<NamedTempFile>`.
//!
//! The spill tempfile is placed in the *target's parent directory* (not
//! `$TMPDIR`) so that the eventual atomic rename in [`crate::atomic`]
//! works without crossing a filesystem boundary (HINT-002).

use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

use tempfile::NamedTempFile;

/// Read chunk size for the drain loop. 64 KiB matches typical filesystem
/// readahead and pages cleanly on all supported targets (HINT-005).
const READ_CHUNK_SIZE: usize = 64 * 1024;

/// The buffered input. Transitions from in-memory to spilled as data accumulates.
pub enum Buffer {
    /// In-memory accumulation (small inputs).
    InMemory(Vec<u8>),
    /// Spilled-to-tempfile accumulation (large inputs).
    Spilled {
        writer: BufWriter<NamedTempFile>,
        /// Tracked size in bytes (since BufWriter writes are not all flushed yet).
        len: u64,
    },
}

impl Buffer {
    /// Construct an empty in-memory buffer with no preallocation.
    pub fn new() -> Self {
        Buffer::InMemory(Vec::new())
    }

    /// Return the current logical length of the buffered bytes.
    pub fn len(&self) -> u64 {
        match self {
            Buffer::InMemory(v) => v.len() as u64,
            Buffer::Spilled { len, .. } => *len,
        }
    }

    /// True iff no bytes have been appended to this buffer.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Drain the entire reader into this buffer, transitioning to the
    /// spilled variant when the accumulated size would exceed `threshold`.
    ///
    /// `spill_dir` is the directory in which the spill tempfile is created
    /// when the transition triggers. Caller MUST pass the *target file's
    /// parent directory* (HINT-002).
    ///
    /// On the binary path (feature `cli`), the loop polls the process-wide
    /// cancellation flag between chunks; if a signal was delivered, the
    /// drain returns `io::ErrorKind::Interrupted` so the in-progress
    /// tempfile is dropped via the normal `Drop` chain before exit.
    pub fn drain_reader<R: Read>(
        &mut self,
        mut reader: R,
        threshold: usize,
        spill_dir: &Path,
    ) -> io::Result<()> {
        let mut chunk = vec![0u8; READ_CHUNK_SIZE];
        loop {
            #[cfg(feature = "cli")]
            if crate::signal::is_cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "rusty-sponge: cancelled by signal",
                ));
            }
            let n = reader.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            self.append(&chunk[..n], threshold, spill_dir)?;
        }
        Ok(())
    }

    /// Append a slice of bytes, transitioning to spilled storage if doing so
    /// would push the buffer past `threshold`.
    pub fn append(&mut self, bytes: &[u8], threshold: usize, spill_dir: &Path) -> io::Result<()> {
        let threshold_u64 = threshold as u64;
        let projected_len = self.len() + bytes.len() as u64;

        // If we are in-memory and the projected size crosses the threshold,
        // spill before writing the new bytes.
        if matches!(self, Buffer::InMemory(_)) && projected_len > threshold_u64 {
            self.transition_to_spilled(spill_dir)?;
        }

        match self {
            Buffer::InMemory(v) => v.extend_from_slice(bytes),
            Buffer::Spilled { writer, len } => {
                writer.write_all(bytes)?;
                *len += bytes.len() as u64;
            }
        }
        Ok(())
    }

    /// Promote an `InMemory` buffer to a `Spilled` one, flushing the existing
    /// bytes into the new tempfile. No-op if already spilled.
    pub fn transition_to_spilled(&mut self, spill_dir: &Path) -> io::Result<()> {
        if let Buffer::InMemory(bytes) = std::mem::replace(self, Buffer::InMemory(Vec::new())) {
            let tempfile = tempfile::Builder::new()
                .prefix(".rusty-sponge-spill-")
                .tempfile_in(spill_dir)?;
            let mut writer = BufWriter::with_capacity(READ_CHUNK_SIZE, tempfile);
            writer.write_all(&bytes)?;
            let len = bytes.len() as u64;
            *self = Buffer::Spilled { writer, len };
        }
        Ok(())
    }

    /// Consume the buffer and write its bytes to `out`. Implementations:
    /// - `InMemory`: a single `write_all` of the Vec.
    /// - `Spilled`: flush BufWriter, rewind the NamedTempFile to start,
    ///   copy through to `out` in 64 KiB chunks.
    pub fn write_to<W: Write>(self, mut out: W) -> io::Result<()> {
        match self {
            Buffer::InMemory(v) => out.write_all(&v),
            Buffer::Spilled { writer, .. } => {
                let mut tempfile = writer
                    .into_inner()
                    .map_err(|e| io::Error::other(format!("BufWriter flush failed: {e}")))?;
                tempfile.as_file_mut().seek(SeekFrom::Start(0))?;
                let mut chunk = vec![0u8; READ_CHUNK_SIZE];
                let mut reader = tempfile.as_file();
                loop {
                    let n = reader.read(&mut chunk)?;
                    if n == 0 {
                        break;
                    }
                    out.write_all(&chunk[..n])?;
                }
                Ok(())
            }
        }
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn empty_buffer_has_len_zero() {
        let buf = Buffer::new();
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn drain_small_input_stays_in_memory() {
        let tmpdir = tempfile::tempdir().unwrap();
        let mut buf = Buffer::new();
        let input = Cursor::new(b"hello world\n");
        buf.drain_reader(input, 1024 * 1024, tmpdir.path()).unwrap();
        assert!(matches!(buf, Buffer::InMemory(_)));
        assert_eq!(buf.len(), 12);
    }

    #[test]
    fn drain_large_input_transitions_to_spilled() {
        let tmpdir = tempfile::tempdir().unwrap();
        let mut buf = Buffer::new();
        // 256 KiB input with 64 KiB threshold → must spill
        let big = vec![0xAAu8; 256 * 1024];
        buf.drain_reader(Cursor::new(&big), 64 * 1024, tmpdir.path())
            .unwrap();
        assert!(matches!(buf, Buffer::Spilled { .. }));
        assert_eq!(buf.len(), 256 * 1024);
    }

    #[test]
    fn write_to_roundtrips_in_memory() {
        let tmpdir = tempfile::tempdir().unwrap();
        let mut buf = Buffer::new();
        buf.drain_reader(Cursor::new(b"abc\n"), 1024 * 1024, tmpdir.path())
            .unwrap();
        let mut out = Vec::new();
        buf.write_to(&mut out).unwrap();
        assert_eq!(out, b"abc\n");
    }

    #[test]
    fn write_to_roundtrips_spilled() {
        let tmpdir = tempfile::tempdir().unwrap();
        let mut buf = Buffer::new();
        let big = (0u8..=255u8).cycle().take(256 * 1024).collect::<Vec<_>>();
        buf.drain_reader(Cursor::new(&big), 1024, tmpdir.path())
            .unwrap();
        assert!(matches!(buf, Buffer::Spilled { .. }));
        let mut out = Vec::new();
        buf.write_to(&mut out).unwrap();
        assert_eq!(out, big);
    }

    #[test]
    fn binary_bytes_pass_through_unchanged() {
        let tmpdir = tempfile::tempdir().unwrap();
        let mut buf = Buffer::new();
        let bytes: &[u8] = &[0x00, 0xFE, 0xFF, 0xC3, 0x28, 0xA0, 0xA1];
        buf.drain_reader(Cursor::new(bytes), 1024 * 1024, tmpdir.path())
            .unwrap();
        let mut out = Vec::new();
        buf.write_to(&mut out).unwrap();
        assert_eq!(out, bytes);
    }

    #[test]
    fn empty_input_writes_zero_bytes() {
        let tmpdir = tempfile::tempdir().unwrap();
        let mut buf = Buffer::new();
        buf.drain_reader(Cursor::new(&[][..]), 1024 * 1024, tmpdir.path())
            .unwrap();
        let mut out = Vec::new();
        buf.write_to(&mut out).unwrap();
        assert_eq!(out, Vec::<u8>::new());
    }
}

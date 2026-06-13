//! Request sequence-number provider (spec §4.4.5).
//!
//! The HDM enforces strictly-increasing sequence numbers per session as an anti-replay defence.
//! The crate models this as a trait so consumers can plug in their own persistence backend
//! (`Redis`, `SQLite`, etc.); a basic in-memory and atomic file-backed implementation ship in-tree.
//!
//! # Choosing an implementation
//!
//! - For single-process, single-session use, `InMemorySeq` is sufficient.
//! - For agents that must survive restarts without colliding with a previously-sent number,
//!   use `FileSeq`. Cost: one atomic file write per operation.
//! - For multi-process / clustered consumers, implement [`SequenceProvider`] yourself against
//!   your shared store. Take an exclusive lock around the increment-and-persist sequence.

use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Provider of monotonically-increasing sequence numbers for HDM requests.
pub trait SequenceProvider: Send {
    /// Advance and return the next sequence number.
    ///
    /// The returned value must be strictly greater than every previously-returned value across
    /// the lifetime of the provider. Successive calls need not produce contiguous values;
    /// "wasted" numbers caused by client-side failures are acceptable.
    ///
    /// # Errors
    /// Implementations may surface I/O errors from their persistence backend. The default
    /// in-memory implementation never fails.
    fn next(&mut self) -> Result<i64, io::Error>;
}

/// In-memory sequence counter.
///
/// Resets on process restart — use only when the consumer either performs a fresh `login`
/// after restart (which establishes a new HDM session) or doesn't care about cross-restart
/// continuity.
#[derive(Debug)]
pub struct InMemorySeq {
    current: i64,
}

impl InMemorySeq {
    /// Start the counter so that the first call to [`Self::next`] returns `initial + 1`.
    #[must_use]
    pub const fn starting_at(initial: i64) -> Self {
        Self { current: initial }
    }

    /// Start the counter so that the first call to [`Self::next`] returns `1`.
    #[must_use]
    pub const fn from_zero() -> Self {
        Self::starting_at(0)
    }
}

impl Default for InMemorySeq {
    fn default() -> Self {
        Self::from_zero()
    }
}

impl SequenceProvider for InMemorySeq {
    fn next(&mut self) -> Result<i64, io::Error> {
        self.current = self
            .current
            .checked_add(1)
            .ok_or_else(|| io::Error::other("sequence counter overflowed i64"))?;
        Ok(self.current)
    }
}

/// File-backed sequence counter that survives restarts, including power loss.
///
/// Each write goes to a sibling temp file that is `fsync`'d, atomically renamed over the target,
/// and (on Unix) followed by an `fsync` of the parent directory so the rename itself is durable.
/// This matters because the counter is a fiscal anti-replay guard — a value that silently reverted
/// after a crash would re-issue a sequence number the device already saw (rejected as
/// `BadSequenceNumber`).
///
/// The file's content is a single decimal integer (UTF-8, optionally followed by whitespace).
/// On open, missing files initialise to `0`; corrupted contents are surfaced as
/// [`io::ErrorKind::InvalidData`] rather than silently reset.
///
/// **This is not safe to share between processes.** The crate does not take an OS-level file
/// lock. Consumers running multiple processes against the same counter file must coordinate
/// externally.
#[derive(Debug)]
pub struct FileSeq {
    path: PathBuf,
    current: i64,
}

impl FileSeq {
    /// Open or create a counter file. If the file exists, its contents are parsed as the last-
    /// returned value; otherwise the counter starts at zero.
    ///
    /// # Errors
    /// Surfaces filesystem failures and parsing failures (corrupted file contents).
    pub fn open_or_create(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let current = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            raw.trim().parse::<i64>().map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("seq counter file is corrupted: {err}"),
                )
            })?
        } else {
            0
        };
        Ok(Self { path, current })
    }

    /// Most-recently-issued value (without advancing the counter).
    #[must_use]
    pub const fn current(&self) -> i64 {
        self.current
    }

    fn persist(&self) -> io::Result<()> {
        // Append ".tmp" to the *full* file name (not `with_extension`, which would strip a real
        // extension and could collide, e.g. `seq.dat` and `seq.bak` both -> `seq.seq.tmp`).
        let mut tmp = self.path.clone().into_os_string();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);

        // Write + fsync the data before swapping it in.
        {
            let mut file = fs::File::create(&tmp)?;
            file.write_all(self.current.to_string().as_bytes())?;
            file.sync_all()?;
        }
        fs::rename(&tmp, &self.path)?;

        // fsync the parent directory so the rename survives a crash. Directory fsync is a Unix
        // facility; on other platforms the file fsync + atomic rename is the best portable effort.
        #[cfg(unix)]
        {
            let dir = self
                .path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            fs::File::open(dir)?.sync_all()?;
        }
        Ok(())
    }
}

impl SequenceProvider for FileSeq {
    fn next(&mut self) -> io::Result<i64> {
        let next = self
            .current
            .checked_add(1)
            .ok_or_else(|| io::Error::other("sequence counter overflowed i64"))?;
        self.current = next;
        self.persist()?;
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::path::PathBuf;

    /// Unique temp path for each test run — uses thread ID + nanos to avoid collisions when
    /// `cargo test` runs in parallel.
    fn unique_tempfile(suffix: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let tid = std::thread::current().id();
        env::temp_dir().join(format!("hdm-am-seq-test-{tid:?}-{nanos}-{suffix}"))
    }

    /// In-memory: first call returns `initial + 1`, then increments monotonically.
    #[test]
    fn in_memory_seq_starts_at_initial_plus_one() {
        let mut seq = InMemorySeq::starting_at(42);
        assert_eq!(seq.next().unwrap(), 43);
        assert_eq!(seq.next().unwrap(), 44);
        assert_eq!(seq.next().unwrap(), 45);
    }

    /// In-memory default starts at 0 → first next is 1.
    #[test]
    fn in_memory_seq_default_starts_at_one() {
        let mut seq = InMemorySeq::default();
        assert_eq!(seq.next().unwrap(), 1);
    }

    /// Overflow at `i64::MAX` surfaces as `io::Error`, not a silent wrap or panic.
    #[test]
    fn in_memory_seq_overflows_safely() {
        let mut seq = InMemorySeq::starting_at(i64::MAX);
        let err = seq.next().expect_err("expected overflow");
        assert_eq!(err.kind(), io::ErrorKind::Other);
    }

    /// File-backed: persists the counter across re-opens.
    #[test]
    fn file_seq_persists_across_reopen() {
        let path = unique_tempfile("persist");
        let _ = fs::remove_file(&path);

        {
            let mut seq = FileSeq::open_or_create(&path).unwrap();
            assert_eq!(seq.next().unwrap(), 1);
            assert_eq!(seq.next().unwrap(), 2);
            assert_eq!(seq.next().unwrap(), 3);
        }

        // Re-open: counter must continue from 3, not reset.
        {
            let mut seq = FileSeq::open_or_create(&path).unwrap();
            assert_eq!(seq.current(), 3);
            assert_eq!(seq.next().unwrap(), 4);
        }

        let _ = fs::remove_file(&path);
    }

    /// File-backed: missing file starts the counter at zero (first next = 1).
    #[test]
    fn file_seq_starts_at_zero_when_missing() {
        let path = unique_tempfile("missing");
        let _ = fs::remove_file(&path);

        let mut seq = FileSeq::open_or_create(&path).unwrap();
        assert_eq!(seq.current(), 0);
        assert_eq!(seq.next().unwrap(), 1);

        let _ = fs::remove_file(&path);
    }

    /// Corrupted file contents surface as `InvalidData` — never a silent reset.
    #[test]
    fn file_seq_refuses_to_open_corrupted_file() {
        let path = unique_tempfile("corrupt");
        fs::write(&path, "this is not a number").unwrap();

        let err = FileSeq::open_or_create(&path).expect_err("expected parse error");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);

        let _ = fs::remove_file(&path);
    }

    /// File-backed: after `next()`, the on-disk content reflects the new value.
    #[test]
    fn file_seq_persists_to_disk_each_call() {
        let path = unique_tempfile("disk");
        let _ = fs::remove_file(&path);

        let mut seq = FileSeq::open_or_create(&path).unwrap();
        seq.next().unwrap();
        seq.next().unwrap();
        seq.next().unwrap();

        let raw = fs::read_to_string(&path).unwrap();
        assert_eq!(raw.trim(), "3");

        let _ = fs::remove_file(&path);
    }
}

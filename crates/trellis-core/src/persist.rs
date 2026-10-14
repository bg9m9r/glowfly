//! Crash-safe writes for Trellis's own files (settings, recent repositories, open tabs).
//!
//! A file is written to a temporary sibling, flushed to disk, then renamed over the target, so a
//! crash or power cut leaves either the old file or the new one, never half of each.
//! [`LatestOnly`] keeps saves that run on separate threads from landing out of order.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// Replaces `path` with `bytes` atomically, creating its folder if needed.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let tmp = temp_sibling(path);
    let written = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()
    })();
    let result = written.and_then(|()| replace(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// A temporary name next to `path` that no other writer (thread or process) is using.
fn temp_sibling(path: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!(".{name}.{}.{n}.tmp", std::process::id()))
}

/// Renames `from` over `to`. `std::fs::rename` replaces an existing target on every platform
/// (on Windows through `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`). Windows refuses while
/// another program (a virus scanner, an indexer) briefly holds the target open, so that is
/// retried a few times.
fn replace(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut attempt = 0;
    loop {
        match std::fs::rename(from, to) {
            Err(e) if cfg!(windows) && attempt < 5 && e.kind() == std::io::ErrorKind::PermissionDenied => {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(20 * attempt));
            }
            other => return other,
        }
    }
}

/// Moves `path` aside to the first free `<name>.bak`, `<name>.bak-2`, `<name>.bak-3`, … and
/// returns the new name.
pub fn move_aside(path: &Path) -> std::io::Result<PathBuf> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let backup = (1..)
        .map(|i| path.with_file_name(if i == 1 { format!("{name}.bak") } else { format!("{name}.bak-{i}") }))
        .find(|p| !p.exists())
        .expect("an unused backup name");
    std::fs::rename(path, &backup)?;
    Ok(backup)
}

/// Orders saves of one file that run on separate threads: each save takes a ticket when it is
/// requested, and a save whose ticket is older than one already written is skipped, so the
/// newest snapshot always wins.
#[derive(Default)]
pub struct LatestOnly {
    issued: AtomicU64,
    written: Mutex<u64>,
}

impl LatestOnly {
    /// Takes the next ticket. Call this where saves are requested, in request order.
    pub fn ticket(&self) -> u64 {
        self.issued.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Runs `write` unless a newer ticket has already been written; `None` when skipped.
    /// Writes are one at a time.
    pub fn write<T>(&self, ticket: u64, write: impl FnOnce() -> T) -> Option<T> {
        let mut written = self.written.lock().unwrap_or_else(|e| e.into_inner());
        if ticket <= *written {
            return None;
        }
        let out = write();
        *written = ticket;
        Some(out)
    }
}

#[cfg(test)]
mod tests;

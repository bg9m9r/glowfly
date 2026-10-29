//! The extensions an index file carries after its entries (`TREE`, `UNTR`, `FSMN`, …), read
//! from the file itself: libgit2 writes only the ones it knows, so git's untracked cache
//! (`UNTR`) and file system monitor token (`FSMN`) can be lost by an index write Glowfly makes
//! through libgit2, and the status read after it is slow again until git records them anew. A
//! trusted whole status read then lets git write the index once (`should_record`), remembering
//! in the runner's `RecordMemo` that it tried, so a record that brings nothing back isn't
//! repeated until the index changes.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::SystemTime;

/// git's untracked cache.
pub const UNTRACKED_CACHE: &str = "UNTR";
/// The file system monitor's token.
pub const MONITOR_TOKEN: &str = "FSMN";

/// The largest index file read for its extensions on a status read: the entries are walked to
/// find them, and past this a read would cost more than it saves.
const MAX_INDEX_BYTES: u64 = 64 << 20;

/// The signatures of the extensions in the index file at `file`, in file order. None when the
/// file can't be read, is larger than `MAX_INDEX_BYTES`, or isn't an index this reader knows
/// (versions 2 to 4, SHA-1 or SHA-256).
pub fn index_extensions(file: &Path) -> Option<Vec<String>> {
    if std::fs::metadata(file).ok()?.len() > MAX_INDEX_BYTES {
        return None;
    }
    let bytes = std::fs::read(file).ok()?;
    [20, 32].into_iter().find_map(|hash| extensions_in(&bytes, hash))
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

/// The extensions of the index `b`, whose object names are `hash` bytes long; None unless
/// the entries and extensions end exactly at the trailing checksum.
fn extensions_in(b: &[u8], hash: usize) -> Option<Vec<String>> {
    if b.get(..4)? != b"DIRC" {
        return None;
    }
    let version = be32(b, 4)?;
    let count = be32(b, 8)?;
    if !(2..=4).contains(&version) {
        return None;
    }
    let end = b.len().checked_sub(hash)?;
    let mut at = 12;
    // ctime, mtime, dev, ino, mode, uid, gid, size (4 bytes each), then the object name.
    let fixed = 40 + hash;
    for _ in 0..count {
        let start = at;
        let flags = u16::from_be_bytes(b.get(start + fixed..start + fixed + 2)?.try_into().ok()?);
        at = start + fixed + 2;
        if version >= 3 && flags & 0x4000 != 0 {
            at += 2;
        }
        if version == 4 {
            // The number of bytes taken off the previous path, as git's offset varint.
            while *b.get(at)? & 0x80 != 0 {
                at += 1;
            }
            at += 1;
        }
        let nul = b.get(at..end)?.iter().position(|&c| c == 0)?;
        at += nul + 1;
        if version < 4 {
            // Padded with NULs to a multiple of eight bytes from the entry's start.
            let len = at - start;
            at = start + len.div_ceil(8) * 8;
        }
    }
    let mut found = Vec::new();
    while at < end {
        let sig = b.get(at..at.checked_add(4)?)?;
        let size = usize::try_from(be32(b, at.checked_add(4)?)?).ok()?;
        found.push(String::from_utf8_lossy(sig).into_owned());
        at = at.checked_add(8)?.checked_add(size)?;
    }
    (at == end).then_some(found)
}

/// An index file's size and modification time: a record already tried is not tried again
/// until the index changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IndexStamp {
    len: u64,
    modified: Option<SystemTime>,
}

impl IndexStamp {
    fn of(file: &Path) -> Option<IndexStamp> {
        let m = std::fs::metadata(file).ok()?;
        Some(IndexStamp { len: m.len(), modified: m.modified().ok() })
    }
}

/// The most index files whose last record is remembered.
const MAX_REMEMBERED: usize = 64;

/// Which index files a status read already let git record its speed-ups in, as they were after
/// that read, so a record that brought nothing back isn't repeated on every refresh. The
/// runner owns one and lends it to its whole status reads (as it does the graph cache).
#[derive(Default)]
pub struct RecordMemo {
    tried: Mutex<Vec<(PathBuf, IndexStamp)>>,
    records: AtomicUsize,
}

impl RecordMemo {
    /// How many reads let git record the speed-ups (tests, the debug log).
    pub fn records(&self) -> usize {
        self.records.load(Ordering::Relaxed)
    }

    fn tried(&self) -> MutexGuard<'_, Vec<(PathBuf, IndexStamp)>> {
        self.tried.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Whether the index at `file`, as it is now (`stamp`), was already left by a record.
    fn already(&self, file: &Path, stamp: IndexStamp) -> bool {
        self.tried().iter().any(|(f, s)| f == file && *s == stamp)
    }

    /// Remembers the index at `file` as it is now, after a record (or a decision not to try).
    fn remember(&self, file: &Path) {
        let Some(stamp) = IndexStamp::of(file) else { return };
        let mut tried = self.tried();
        tried.retain(|(f, _)| f != file);
        if tried.len() >= MAX_REMEMBERED {
            tried.remove(0);
        }
        tried.push((file.to_path_buf(), stamp));
    }
}

/// What a whole status read needs to know to record the speed-ups again (`record_after`).
pub(super) struct Lost {
    file: PathBuf,
}

/// Whether the whole status read of `repo` about to run should let git write the index, so it
/// records again a speed-up its config turns on but the index no longer has: the untracked
/// cache (kept for Glowfly's `--untracked-files=all` read unless `status.showUntrackedFiles`
/// says otherwise), or the token of git's own file system monitor daemon, but only where a
/// daemon can give one (`fsmonitor::daemon_serves`: a Watchman hook, a git without the daemon,
/// Linux or a network drive never would). Not again for an index `memo` saw after a record,
/// until it changes. None when there is nothing to record, the index can't be read here (too
/// large, or a split index), or no memo was lent.
pub(super) fn should_record(repo: &git2::Repository, memo: Option<&RecordMemo>) -> Option<Lost> {
    let memo = memo?;
    let config = repo.config().ok()?;
    let s = super::status_speed::speedups_of(&config);
    let wants_cache = s.untracked_cache && s.show_untracked_all != Some(false);
    let daemon = config.get_bool("core.fsmonitor").unwrap_or(false);
    if !wants_cache && !daemon {
        return None;
    }
    let file = repo.index().ok()?.path()?.to_path_buf();
    let stamp = IndexStamp::of(&file)?;
    if memo.already(&file, stamp) {
        return None;
    }
    let found = index_extensions(&file)?;
    // A split index keeps the entries elsewhere; git records the extensions in its own way.
    if found.iter().any(|e| e == "link") {
        return None;
    }
    let has = |sig: &str| found.iter().any(|e| e == sig);
    let cache_lost = wants_cache && !has(UNTRACKED_CACHE);
    let token_lost = daemon && !has(MONITOR_TOKEN);
    // Asked of git once per index state at most: remembered whatever the answer.
    let serves = || repo.workdir().is_some_and(super::fsmonitor::daemon_serves);
    if cache_lost || (token_lost && serves()) {
        return Some(Lost { file });
    }
    if token_lost {
        memo.remember(&file);
    }
    None
}

/// After a read that let git record (`lost`): counts it and remembers the index as git left it.
pub(super) fn record_after(memo: Option<&RecordMemo>, lost: &Lost) {
    if let Some(memo) = memo {
        memo.records.fetch_add(1, Ordering::Relaxed);
        memo.remember(&lost.file);
    }
}

#[cfg(test)]
#[path = "index_ext_tests.rs"]
mod tests;

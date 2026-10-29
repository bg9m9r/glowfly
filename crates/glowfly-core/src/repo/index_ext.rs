//! The extensions an index file carries after its entries (`TREE`, `UNTR`, `FSMN`, …), read
//! from the file itself: libgit2 writes only the ones it knows, so git's untracked cache
//! (`UNTR`) and file system monitor token (`FSMN`) can be lost by an index write Glowfly makes
//! through libgit2, and the status read after it is slow again until git records them anew.

use std::path::Path;

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
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
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
        let sig = b.get(at..at + 4)?;
        let size = usize::try_from(be32(b, at + 4)?).ok()?;
        found.push(String::from_utf8_lossy(sig).into_owned());
        at = at.checked_add(8 + size)?;
    }
    (at == end).then_some(found)
}

/// Whether `repo`'s config turns on a speed-up its index no longer records: the untracked cache
/// (kept for Glowfly's `--untracked-files=all` read unless `status.showUntrackedFiles` says
/// otherwise) or the token of git's file system monitor daemon. A whole status read by git then lets git write
/// the index, so it records them again; once they are back, reads take no index lock again.
/// False when the index can't be read here (too large, or a split index).
pub(super) fn lost_speedups(repo: &git2::Repository) -> bool {
    let Ok(config) = repo.config() else { return false };
    let s = super::status_speed::speedups_of(&config);
    let wants_cache = s.untracked_cache && s.show_untracked_all != Some(false);
    // Only git's own daemon: a hook (Watchman's) that isn't running never gives a token, and
    // every read would then take the index lock.
    let daemon = config.get_bool("core.fsmonitor").unwrap_or(false);
    if !wants_cache && !daemon {
        return false;
    }
    let Some(found) = repo.index().ok().and_then(|i| i.path().map(Path::to_path_buf)).and_then(|f| index_extensions(&f)) else {
        return false;
    };
    // A split index keeps the entries elsewhere; git records the extensions in its own way.
    if found.iter().any(|e| e == "link") {
        return false;
    }
    let has = |sig: &str| found.iter().any(|e| e == sig);
    (wants_cache && !has(UNTRACKED_CACHE)) || (daemon && !has(MONITOR_TOKEN))
}

#[cfg(test)]
#[path = "index_ext_tests.rs"]
mod tests;

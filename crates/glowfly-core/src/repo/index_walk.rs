//! A walk over the entries of an index file's bytes (gitformat-index versions 2 to 4, SHA-1 or
//! SHA-256), for the readers that want only some entries' modes and paths: libgit2's own parse
//! of a large index (its entry map, the checksum) costs far more than reading what they need.
//!
//! A split index (`link`: the entries live in a shared file) or a sparse one (`sdir`: folder
//! entries) is not walked: the caller falls back to libgit2, which knows them.

use std::path::Path;

/// The largest index file read whole by `entries_of_file`.
const MAX_BYTES: u64 = 256 << 20;

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

/// The mode libgit2 gives an entry read from the file: a link, a folder or a gitlink as it is,
/// anything else a regular file with git's two permission forms.
fn mode_of(raw: u32) -> u32 {
    match raw & 0o170000 {
        kind @ (0o120000 | 0o040000 | 0o160000) => kind,
        _ if raw & 0o100 != 0 => 0o100755,
        _ => 0o100644,
    }
}

/// Calls `visit(mode, path)` for each entry of the index `b` (object names `hash` bytes long) in
/// file order. Returns where the entries end (the first extension), or None unless the header is
/// right and every entry lies inside the file, before its trailing checksum.
pub(super) fn walk(b: &[u8], hash: usize, mut visit: impl FnMut(u32, &[u8])) -> Option<usize> {
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
    let mut path: Vec<u8> = Vec::new();
    for _ in 0..count {
        let start = at;
        let mode = mode_of(be32(b, start + 24)?);
        let flags = u16::from_be_bytes(b.get(start + fixed..start + fixed + 2)?.try_into().ok()?);
        at = start + fixed + 2;
        if version >= 3 && flags & 0x4000 != 0 {
            at += 2;
        }
        if version == 4 {
            // The number of bytes taken off the previous path, as git's offset varint.
            let mut strip = usize::from(*b.get(at)? & 0x7f);
            while *b.get(at)? & 0x80 != 0 {
                at += 1;
                strip = ((strip + 1) << 7) | usize::from(*b.get(at)? & 0x7f);
            }
            at += 1;
            let nul = b.get(at..end)?.iter().position(|&c| c == 0)?;
            path.truncate(path.len().checked_sub(strip)?);
            path.extend_from_slice(&b[at..at + nul]);
            at += nul + 1;
            visit(mode, &path);
        } else {
            let nul = b.get(at..end)?.iter().position(|&c| c == 0)?;
            visit(mode, &b[at..at + nul]);
            at += nul + 1;
            // Padded with NULs to a multiple of eight bytes from the entry's start.
            at = start + (at - start).div_ceil(8) * 8;
        }
    }
    (at <= end).then_some(at)
}

/// The (mode, path) of the entries of the index `bytes` that `keep` accepts, in file order.
/// None for a file this walk doesn't read: not an index, cut short, or split or sparse.
pub(super) fn entries(bytes: &[u8], mut keep: impl FnMut(u32, &[u8]) -> bool) -> Option<Vec<(u32, Vec<u8>)>> {
    [20, 32].into_iter().find_map(|hash| {
        let mut out = Vec::new();
        let at = walk(bytes, hash, |mode, path| {
            if keep(mode, path) {
                out.push((mode, path.to_vec()));
            }
        })?;
        let found = super::index_ext::extensions_from(bytes, at, bytes.len() - hash)?;
        (!found.iter().any(|e| e == "link" || e == "sdir")).then_some(out)
    })
}

/// `entries` of the index file `file`; None also when it can't be read or is over `MAX_BYTES`.
pub(crate) fn entries_of_file(file: &Path, keep: impl FnMut(u32, &[u8]) -> bool) -> Option<Vec<(u32, Vec<u8>)>> {
    if std::fs::metadata(file).ok()?.len() > MAX_BYTES {
        return None;
    }
    entries(&std::fs::read(file).ok()?, keep)
}

#[cfg(test)]
#[path = "index_walk_tests.rs"]
mod tests;

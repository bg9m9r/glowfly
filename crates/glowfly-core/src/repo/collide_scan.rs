//! Finding the index's collisions (`collide.rs`) cheaply, since every status read asks, the
//! watcher's path-limited reads after each save included.
//!
//! The scan hashes each path's key once (an ASCII path, nearly all of them, by its lowercased
//! bytes with no normalization and no allocation) and only builds the real keys for the few
//! paths whose hash another path already has. Its answer is kept per index file, by the file's
//! length, modification time and trailing checksum, so reads of an unchanged index don't scan
//! at all. A stamp is only trusted when the file was last written at least `RACY_WINDOW` before
//! it was taken (git's "racy git" rule), and only kept when the file had the same stamp before
//! and after the index in memory was brought up to date and scanned.

use super::Collision;
use git2::Index;
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{BuildHasherDefault, Hasher};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

/// The index files whose answers are kept; the oldest goes first.
const KEPT: usize = 32;

/// The groups of index paths (any stage, UTF-8 names only) that share a key, sorted: from the
/// cache when the index file is unchanged since it was last scanned, else scanned (and kept).
pub(in crate::repo) fn collisions(index: &mut Index) -> Vec<Collision> {
    let Some(file) = index.path().map(Path::to_path_buf) else { return scan(index) };
    let before = stamp(&file);
    if let Some(found) = before.as_ref().and_then(|s| cached(&file, s)) {
        return found;
    }
    // So the copy in memory is the file the stamp describes.
    let _ = index.read(false);
    let found = scan(index);
    if let Some(s) = before
        && stamp(&file).as_ref() == Some(&s)
    {
        keep(file, s, found.clone());
    }
    found
}

/// The index file as it stood when its answer was kept.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: SystemTime,
    /// The checksum git writes at the end of the file (zeros with `index.skipHash`).
    trailer: [u8; 20],
}

/// The stamp of `file`; None when it can't be read or was written too recently to trust.
fn stamp(file: &Path) -> Option<Stamp> {
    let mut f = std::fs::File::open(file).ok()?;
    let meta = f.metadata().ok()?;
    let modified = meta.modified().ok()?;
    if !crate::repo::stamp_trustworthy(Some(modified), SystemTime::now()) || meta.len() < 20 {
        return None;
    }
    let mut trailer = [0; 20];
    f.seek(SeekFrom::End(-20)).ok()?;
    f.read_exact(&mut trailer).ok()?;
    Some(Stamp { len: meta.len(), modified, trailer })
}

static CACHE: Mutex<Vec<(PathBuf, Stamp, Vec<Collision>)>> = Mutex::new(Vec::new());

fn cached(file: &Path, s: &Stamp) -> Option<Vec<Collision>> {
    let cache = CACHE.lock().ok()?;
    cache.iter().find(|(f, st, _)| f == file && st == s).map(|(_, _, found)| found.clone())
}

fn keep(file: PathBuf, s: Stamp, found: Vec<Collision>) {
    let Ok(mut cache) = CACHE.lock() else { return };
    cache.retain(|(f, _, _)| *f != file);
    if cache.len() >= KEPT {
        cache.remove(0);
    }
    cache.push((file, s, found));
}

/// `collisions` without the cache.
fn scan(index: &Index) -> Vec<Collision> {
    #[cfg(test)]
    tests::SCANS.with(|n| n.set(n.get() + 1));
    // Each key's hash -> the first entry with it; entries whose hash was already there, and
    // those first entries, are the suspects.
    let mut first: HashMap<u64, usize, BuildHasherDefault<Prehashed>> = HashMap::default();
    first.reserve(index.len());
    let mut suspects = Vec::new();
    for i in 0..index.len() {
        let Some(h) = index.get(i).and_then(|e| key_hash(&e.path)) else { continue };
        match first.entry(h) {
            Entry::Occupied(o) => suspects.extend([*o.get(), i]),
            Entry::Vacant(v) => {
                v.insert(i);
            }
        }
    }
    if suspects.is_empty() {
        return Vec::new();
    }
    suspects.sort_unstable();
    suspects.dedup();
    let mut by_key: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for i in suspects {
        if let Some(path) = index.get(i).and_then(|e| String::from_utf8(e.path).ok()) {
            by_key.entry(super::key(&path)).or_default().insert(path);
        }
    }
    by_key
        .into_values()
        .filter(|set| set.len() > 1)
        .map(|set| {
            let composed: BTreeSet<String> = set.iter().map(|p| super::nfc(p)).collect();
            Collision { case: composed.len() > 1, unicode: composed.len() < set.len(), paths: set.into_iter().collect() }
        })
        .collect()
}

/// The hash of `path`'s key (`collide::key`); None for a name that isn't UTF-8. An ASCII path's
/// key is its lowercase, hashed byte by byte; any other is built and hashed the same way, so
/// a non-ASCII name whose key is ASCII (the Kelvin sign's "k") meets its ASCII twin.
fn key_hash(path: &[u8]) -> Option<u64> {
    if path.is_ascii() {
        return Some(fnv(path.iter().map(u8::to_ascii_lowercase)));
    }
    let path = std::str::from_utf8(path).ok()?;
    Some(fnv(super::key(path).bytes()))
}

/// 64-bit FNV-1a: quick on short paths; a clash only makes two entries suspects.
fn fnv(bytes: impl Iterator<Item = u8>) -> u64 {
    bytes.fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// A hasher for keys that are hashes already.
#[derive(Default)]
struct Prehashed(u64);

impl Hasher for Prehashed {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 << 8) | u64::from(*b);
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = n;
    }
}

#[cfg(test)]
#[path = "collide_scan_tests.rs"]
mod tests;

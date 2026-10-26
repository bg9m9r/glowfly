//! A saved copy of each repository's whole-history index, in the cache folder
//! (`<cache>/search-index/`), so a later session (or a tab whose index was freed) reads it back
//! instead of walking the history again.
//!
//! One file per repository, named by its resolved common git dir, so every worktree of it shares
//! the file. A saved index written by this Glowfly version for the same history (`IndexKey`) is
//! used as it is; any other one is only the base of an update (`build` copies what it holds and
//! reads the commits it lacks), because its positions may come from another walk. The layout
//! (`format.rs`) carries a `FORMAT` number: a file of another number is ignored and later
//! overwritten. A damaged file is deleted.
//!
//! It is a cache: files are private to the user, written atomically through
//! `persist::write_streamed`, and pruned when unused for `MAX_AGE` and then, oldest first, past
//! `MAX_TOTAL` bytes. Every call here touches the disk: worker threads only.

mod format;

use super::{CommitIndex, IndexKey};
use crate::persist::{self, WriteOptions};
use crate::repo::CommonDir;
use format::ReadError;
use sha2::{Digest, Sha256};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The file layout's version. Bump it when the layout changes, or when what an entry holds
/// changes (the summary, `repo::walk::flatten_body_into`'s flattening or its cap, the author
/// fields): files written before are then ignored and overwritten.
pub(crate) const FORMAT: u32 = 2;

/// A file unused for this long is removed.
const MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);
/// The most the folder holds; the least recently used files past it are removed.
const MAX_TOTAL: u64 = 2 << 30;
/// A temporary file older than this was left by a crash.
const TEMP_AGE: Duration = Duration::from_secs(24 * 60 * 60);
/// How many commits a complete index must have gained or lost since it was saved for the same
/// shallow boundary before it is saved again.
pub const RESAVE_COMMITS: usize = 5_000;

const FOLDER: &str = "search-index";
const EXT: &str = "idx";

/// The folder of saved indexes.
#[derive(Clone, Debug)]
pub struct Store {
    dir: PathBuf,
}

/// The file one repository's index is saved in.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StoreFile {
    pub path: PathBuf,
    /// The repository's resolved common dir, encoded (`paths::encode`): saved in the file and
    /// checked on read.
    pub(crate) repo: String,
}

/// What a saved file holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Stored {
    pub key: IndexKey,
    pub len: usize,
    pub complete: bool,
    /// Written by this Glowfly version (and, for an incomplete index, with today's cap): only
    /// then are its positions trusted.
    pub current: bool,
}

impl Stored {
    /// What saving `ix` now leaves in the file.
    pub fn of(ix: &CommitIndex) -> Stored {
        Stored { key: ix.key().clone(), len: ix.len(), complete: ix.complete(), current: true }
    }
}

impl Store {
    pub fn new(dir: PathBuf) -> Store {
        Store { dir }
    }

    /// `<cache folder>/search-index`, or None without a cache folder. Reads only the
    /// environment.
    pub fn default_dir() -> Option<PathBuf> {
        crate::settings::cache_dir().map(|c| c.join(FOLDER))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The file of the repository at (or above) `workdir`; None where no repository opens.
    pub fn file_of(&self, workdir: &Path) -> Option<StoreFile> {
        let common = CommonDir::at(workdir)?;
        let repo = crate::paths::encode(&common.dir);
        let digest = Sha256::digest(repo.as_bytes());
        let name: String = digest[..16].iter().map(|b| format!("{b:02x}")).collect();
        Some(StoreFile { path: self.dir.join(format!("{name}.{EXT}")), repo })
    }

    /// The index saved in `file`, marking the file used. None when there is none, it was
    /// written in another layout (left for a later save to replace), it is damaged (deleted) or
    /// `stop` said to stop.
    pub fn load(&self, file: &StoreFile, stop: &dyn Fn() -> bool) -> Option<(CommitIndex, Stored)> {
        let _span = tracing::debug_span!("index.load").entered();
        match format::read(&file.path, &file.repo, stop) {
            Ok(found) => {
                if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&file.path) {
                    let _ = f.set_modified(SystemTime::now());
                }
                Some(found)
            }
            Err(ReadError::Damaged(why)) => {
                tracing::debug!("removing the damaged search index {} ({why})", file.path.display());
                let _ = persist::remove_file(&file.path);
                None
            }
            Err(ReadError::OtherFormat | ReadError::Stopped) => None,
            Err(ReadError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(ReadError::Io(e)) => {
                tracing::debug!("could not read the search index {}: {e}", file.path.display());
                None
            }
        }
    }

    /// Saves `ix` in `file` (atomically, private to the user) and says what the file now holds.
    pub fn save(&self, file: &StoreFile, ix: &CommitIndex) -> std::io::Result<Stored> {
        let _span = tracing::debug_span!("index.save", commits = ix.len()).entered();
        let opts = WriteOptions { private: true, ..Default::default() };
        persist::write_streamed(&file.path, opts, |f| format::write(BufWriter::with_capacity(1 << 20, f), &file.repo, ix))?;
        Ok(Stored::of(ix))
    }

    /// Removes files unused for 30 days, then the least recently used past 2 GiB in all (never
    /// `keep`), and temporary files a crash left over a day ago. Returns how many it removed.
    pub fn prune(&self, now: SystemTime, keep: Option<&Path>) -> usize {
        self.prune_with(now, MAX_AGE, MAX_TOTAL, keep)
    }

    pub(crate) fn prune_with(&self, now: SystemTime, max_age: Duration, max_total: u64, keep: Option<&Path>) -> usize {
        let Ok(list) = std::fs::read_dir(&self.dir) else { return 0 };
        let older = |m: &std::fs::Metadata, age: Duration| m.modified().is_ok_and(|t| now.duration_since(t).is_ok_and(|d| d > age));
        let mut removed = 0;
        let mut remove = |p: &Path| {
            if persist::remove_file(p).is_ok() {
                removed += 1;
            }
        };
        let mut saved = Vec::new();
        for entry in list.flatten() {
            let path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
            if !meta.is_file() {
                continue;
            }
            match path.extension().and_then(|e| e.to_str()) {
                Some("tmp") if older(&meta, TEMP_AGE) => remove(&path),
                Some(EXT) if older(&meta, max_age) && keep != Some(path.as_path()) => remove(&path),
                Some(EXT) => saved.push((meta.modified().ok(), meta.len(), path)),
                _ => {}
            }
        }
        // Newest first; `keep` counts first, as the file just written.
        saved.sort_by_key(|s| std::cmp::Reverse(s.0));
        let mut total: u64 = saved.iter().filter(|s| keep == Some(s.2.as_path())).map(|s| s.1).sum();
        for (_, len, path) in &saved {
            if keep == Some(path.as_path()) {
                continue;
            }
            total = total.saturating_add(*len);
            if total > max_total {
                remove(path);
            }
        }
        removed
    }
}

/// Whether a finished index should be saved, given what its file holds (`stored`, None when
/// nothing is known to be there). Pure.
pub fn worth_saving(stored: Option<&Stored>, ix: &CommitIndex) -> bool {
    let Some(s) = stored.filter(|s| s.current) else { return true };
    if s.key == *ix.key() {
        return false;
    }
    // The saved copy can't be the base of this history's update.
    if !ix.complete() || !s.complete || s.key.shallow != ix.key().shallow {
        return true;
    }
    s.len.abs_diff(ix.len()) >= RESAVE_COMMITS
}

/// Test hooks into the layout, for tests that damage a file field by field.
#[cfg(test)]
pub(crate) mod layout {
    pub const HEADER: usize = super::format::HEADER;
    /// The byte offset of the FORMAT field and the header's counts.
    pub const FORMAT_AT: usize = 8;
    pub const ENTRIES_AT: usize = 40;
    pub const BODY_LEN_AT: usize = 24;
    pub const ENTRY: usize = 64;
    pub const OID: usize = 20;

    /// The CRC-32 of `bytes`, as the trailer holds it.
    pub fn crc(bytes: &[u8]) -> u32 {
        let mut c = flate2::Crc::new();
        c.update(bytes);
        c.sum()
    }
}

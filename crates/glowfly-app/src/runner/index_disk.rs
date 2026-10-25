//! The index thread's use of the saved indexes (`commit_index::Store`): before a build without a
//! base it reads the repository's saved index, which is used as it is when it is this version's
//! copy of the same history and is the build's base otherwise; after a build is reported it is
//! saved when `store::worth_saving` says so. The store is pruned once a session before the first
//! read and after every save. The store is attached at startup (`Runner::attach_index_store`),
//! never in tests, so without it builds run as before.

use glowfly_core::commit_index::{self, CommitIndex, IndexKey, Progress, Store, StoreFile, Stored, worth_saving};
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::SystemTime;

/// The store, once attached, and what this session knows of its files.
#[derive(Default)]
pub(super) struct Disk {
    store: OnceLock<Store>,
    state: Mutex<DiskState>,
}

#[derive(Default)]
struct DiskState {
    /// What each file (by path) held when this session last read or wrote it.
    saved: HashMap<PathBuf, Stored>,
    /// The store was pruned this session.
    pruned: bool,
}

/// Where a finished index came from.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(super) enum Source {
    /// Read from the store as it is.
    Loaded,
    /// Built from a base (the tab's previous index or the saved one), reading only new commits.
    Updated,
    /// Built from nothing.
    Built,
}

/// A finished index, and the file to save it in afterwards (if it is worth saving).
pub(super) struct Done {
    pub index: CommitIndex,
    pub source: Source,
    pub save: Option<StoreFile>,
}

impl Disk {
    /// Uses the store in `dir` from now on; a second call is ignored.
    pub(super) fn attach(&self, dir: PathBuf) {
        let _ = self.store.set(Store::new(dir));
    }

    fn state(&self) -> MutexGuard<'_, DiskState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The index of `workdir`'s history walked from `key`: read from the store, or built (from
    /// `base`, else from the saved index when there is one). `Ok(None)` when `stop` said to stop.
    pub(super) fn build(
        &self,
        workdir: &Path,
        key: &IndexKey,
        base: Option<&CommitIndex>,
        progress: &mut dyn FnMut(Progress),
        stop: &dyn Fn() -> bool,
    ) -> glowfly_core::error::Result<Option<Done>> {
        let Some((store, file)) = self.store.get().and_then(|s| Some((s, s.file_of(workdir)?))) else {
            let built = commit_index::build(workdir, key, base, progress, stop)?;
            return Ok(built.map(|index| Done { index, source: source_of(base, key), save: None }));
        };
        {
            let mut state = self.state();
            if !state.pruned {
                state.pruned = true;
                store.prune(SystemTime::now(), None);
            }
        }
        let mut loaded = None;
        if base.is_none() {
            let found = store.load(&file, stop);
            if stop() {
                return Ok(None);
            }
            if let Some((index, stored)) = found {
                self.state().saved.insert(file.path.clone(), stored.clone());
                if stored.current && stored.key == *key {
                    progress(Progress { indexed: index.len(), estimate: None });
                    return Ok(Some(Done { index, source: Source::Loaded, save: None }));
                }
                loaded = Some(index);
            }
        }
        let base = base.or(loaded.as_ref());
        let Some(index) = commit_index::build(workdir, key, base, progress, stop)? else { return Ok(None) };
        let save = worth_saving(self.state().saved.get(&file.path), &index).then_some(file);
        Ok(Some(Done { source: source_of(base, key), index, save }))
    }

    /// Saves `ix` in `file`, then prunes the store (keeping `file`). A failure (a full disk, a
    /// read-only cache, a file another program holds) is logged and leaves the index as it is.
    pub(super) fn save(&self, file: &StoreFile, ix: &CommitIndex) {
        let Some(store) = self.store.get() else { return };
        let saved = catch_unwind(AssertUnwindSafe(|| {
            let stored = store.save(file, ix)?;
            self.state().saved.insert(file.path.clone(), stored);
            store.prune(SystemTime::now(), Some(&file.path));
            Ok::<(), std::io::Error>(())
        }));
        match saved {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!("could not save the search index for {}: {e}", file.path.display()),
            Err(_) => tracing::warn!("saving the search index for {} failed with an internal error", file.path.display()),
        }
    }
}

/// Whether a build from `base` copies from it (`commit_index::build` reuses only a complete base
/// with the same shallow boundary).
fn source_of(base: Option<&CommitIndex>, key: &IndexKey) -> Source {
    if base.is_some_and(|b| b.complete() && b.key().shallow == key.shallow) { Source::Updated } else { Source::Built }
}

#[cfg(test)]
#[path = "index_disk_tests.rs"]
mod tests;

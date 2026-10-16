//! Which paths name one folder, without touching the disk: `update` and the views compare paths
//! by spelling (`paths::same_spelling`) plus the identities (`paths::FolderId`) worker threads
//! found for them, when a repository loaded (`Snapshot::folder`) or the listed folders were
//! checked (`Effect::CheckRepoFolders`). Two spellings with one identity are one folder whatever
//! their case; two with different identities are two, even where the OS usually ignores case.

use super::AppState;
use std::path::{Path, PathBuf};
use trellis_core::paths::{FolderId, same_spelling};
use trellis_core::repo::Snapshot;

/// The most spellings remembered (recent, favorites and tabs need far fewer).
const MAX_KNOWN: usize = 256;

/// Spellings with the identity last found for them, oldest first.
#[derive(Default)]
pub struct KnownFolders {
    seen: Vec<(PathBuf, FolderId)>,
}

impl KnownFolders {
    /// Records that `path` named the folder `id` when last looked at.
    pub fn learn(&mut self, path: &Path, id: FolderId) {
        self.forget(path);
        if self.seen.len() >= MAX_KNOWN {
            self.seen.remove(0);
        }
        self.seen.push((path.to_path_buf(), id));
    }

    /// `path` named nothing when last looked at.
    pub fn forget(&mut self, path: &Path) {
        self.seen.retain(|(p, _)| !same_spelling(p, path));
    }

    /// The identity last found for `path`.
    pub fn id_of(&self, path: &Path) -> Option<FolderId> {
        self.seen.iter().rev().find(|(p, _)| same_spelling(p, path)).map(|(_, id)| *id)
    }

    /// Records every spelling `snap` was seen by.
    pub fn learn_snapshot(&mut self, snap: &Snapshot) {
        let Some(id) = snap.folder else { return };
        for p in [Some(&snap.workdir), Some(&snap.real_workdir), snap.opened_as.as_ref()].into_iter().flatten() {
            self.learn(p, id);
        }
    }
}

impl AppState {
    /// Whether `a` and `b` name the same folder: spelled the same, or found to be one folder.
    pub fn same_folder(&self, a: &Path, b: &Path) -> bool {
        same_spelling(a, b) || self.known.id_of(a).is_some_and(|x| self.known.id_of(b) == Some(x))
    }

    /// Whether `path` names the repository `snap` loaded: one of the spellings it was seen by,
    /// or a path found to be the same folder.
    pub fn names_repo(&self, snap: &Snapshot, path: &Path) -> bool {
        super::tabs::is_repo_at(snap, path) || snap.folder.is_some_and(|f| self.known.id_of(path) == Some(f))
    }

    /// The index of the tab showing the folder at `path`.
    pub fn tab_showing(&self, path: &Path) -> Option<usize> {
        let id = self.known.id_of(path);
        self.tabs.iter().position(|t| t.path.is_some() && t.place().is(path, id))
    }

    /// The path a repository's settings are saved under (`settings::repo_keys`): where the
    /// repository at `workdir` really is, as its last load found (`Snapshot::real_workdir`), or
    /// `workdir` itself before it has loaded.
    pub fn repo_key(&self, workdir: &Path) -> PathBuf {
        if let Some(snap) = self.snap.as_ref().filter(|s| self.names_repo(s, workdir)) {
            return snap.real_workdir.clone();
        }
        let id = self.known.id_of(workdir);
        self.tabs.iter().filter(|t| t.place().is(workdir, id)).find_map(|t| t.real.clone()).unwrap_or_else(|| workdir.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use trellis_core::settings::Settings;

    fn id(n: u64) -> FolderId {
        FolderId::for_tests(n)
    }

    #[test]
    fn identities_decide_where_spelling_cannot() {
        let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
        let (upper, lower, link) = (Path::new("/v/App"), Path::new("/v/app"), Path::new("/l/app"));
        assert!(!st.same_folder(upper, lower), "unknown: only the spelling counts");
        assert!(st.same_folder(upper, Path::new("/v/./App/")));
        // A volume that tells case apart: two folders.
        st.known.learn(upper, id(1));
        st.known.learn(lower, id(2));
        st.known.learn(link, id(2));
        assert!(!st.same_folder(upper, lower));
        assert!(st.same_folder(lower, link));
        // One that ignores case: one folder.
        st.known.learn(upper, id(2));
        assert!(st.same_folder(upper, lower));
        st.known.forget(Path::new("/v/App/"));
        assert_eq!(st.known.id_of(upper), None);
        assert!(!st.same_folder(upper, lower));
    }

    #[test]
    fn the_cache_stays_small() {
        let mut k = KnownFolders::default();
        for i in 0..(MAX_KNOWN as u64 + 10) {
            k.learn(&PathBuf::from(format!("/w/{i}")), id(i));
        }
        assert_eq!(k.seen.len(), MAX_KNOWN);
        assert_eq!(k.id_of(Path::new("/w/0")), None, "the oldest went first");
        assert_eq!(k.id_of(Path::new(&format!("/w/{}", MAX_KNOWN + 9))), Some(id(MAX_KNOWN as u64 + 9)));
    }
}

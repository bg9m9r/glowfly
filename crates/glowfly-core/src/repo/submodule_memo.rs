//! What the submodule list keeps between status reads (`SubmoduleMemo`): each out-of-sync
//! submodule's left-behind commits, walked again only when its gitlink, its HEAD or its refs
//! changed, and counts of the working-tree scans and walks the list made.

use git2::{Oid, Repository};
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};

/// The most left-behind answers kept (of every repository the app reads); past it, the oldest go.
const MAX_KEPT: usize = 256;

/// What a left-behind answer depends on: the submodule's git dir, the recorded commit, the
/// checked-out commit and a fingerprint of the refs that hide commits (`refs_print`).
#[derive(Clone, PartialEq, Eq, Debug)]
struct Key {
    git_dir: PathBuf,
    recorded: Oid,
    head: Oid,
    refs: u64,
}

type Commits = Vec<(Oid, String)>;

/// Kept by the runner (in its `GraphCache`) and lent to every status read's submodule list.
#[derive(Default)]
pub struct SubmoduleMemo {
    kept: Mutex<VecDeque<(Key, Commits)>>,
    scans: AtomicUsize,
    walks: AtomicUsize,
}

impl SubmoduleMemo {
    /// How many submodule working trees the list scanned itself, untracked files included,
    /// because the status read it followed couldn't tell (tests, the debug log).
    pub fn scans(&self) -> usize {
        self.scans.load(Ordering::Relaxed)
    }

    /// How many left-behind walks ran (tests, the debug log).
    pub fn walks(&self) -> usize {
        self.walks.load(Ordering::Relaxed)
    }

    pub(super) fn scanned(&self) {
        self.scans.fetch_add(1, Ordering::Relaxed);
    }

    fn kept(&self) -> MutexGuard<'_, VecDeque<(Key, Commits)>> {
        self.kept.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The left-behind commits of the submodule `repo` checked out at `head` with `recorded`
    /// in the parent's index: the answer kept for the same commits and refs, else `walk`'s,
    /// kept only when `walk` says it may be (a whole walk that only a ref change can change).
    pub(super) fn left_behind(&self, repo: &Repository, recorded: Oid, head: Oid, walk: impl FnOnce() -> (Commits, bool)) -> Commits {
        let Some(refs) = refs_print(repo) else {
            self.walks.fetch_add(1, Ordering::Relaxed);
            return walk().0;
        };
        let key = Key { git_dir: repo.path().to_path_buf(), recorded, head, refs };
        if let Some((_, c)) = self.kept().iter().find(|(k, _)| *k == key) {
            return c.clone();
        }
        self.walks.fetch_add(1, Ordering::Relaxed);
        let (commits, keep) = walk();
        let mut kept = self.kept();
        // One answer per submodule: an older one for other commits or refs won't be asked again soon.
        kept.retain(|(k, _)| k.git_dir != key.git_dir);
        if !keep {
            return commits;
        }
        if kept.len() >= MAX_KEPT {
            kept.pop_front();
        }
        kept.push_back((key, commits.clone()));
        commits
    }
}

/// A fingerprint of the branches, tags and remote-tracking branches (names and targets) that
/// hide commits from the left-behind walk; None when the refs can't be read.
fn refs_print(repo: &Repository) -> Option<u64> {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for r in repo.references().ok()? {
        let r = r.ok()?;
        let name = r.name_bytes();
        if [&b"refs/heads/"[..], b"refs/tags/", b"refs/remotes/"].iter().any(|p| name.starts_with(p)) {
            name.hash(&mut h);
            r.target().hash(&mut h);
            r.symbolic_target_bytes().hash(&mut h);
        }
    }
    Some(h.finish())
}

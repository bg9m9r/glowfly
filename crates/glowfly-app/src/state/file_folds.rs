//! The folders collapsed in the changed-file lists' tree, per list. View state of the active
//! tab: parked with it, cleared when the tab opens another repository, never saved.

use crate::state::{AppState, Selection};
use glowfly_core::repo::DiffTarget;
use glowfly_core::settings::FileListMode;
use std::collections::{BTreeMap, BTreeSet};

/// A changed-file list that keeps its own collapsed folders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FileListId {
    /// The commit details list: one set for every commit, so a fold stays while browsing.
    Commit,
    /// The multi-commit comparison list.
    Compare,
    /// One commit's section of the stacked list.
    Stacked(git2::Oid),
    Unstaged,
    Staged,
}

/// The collapsed folders of each list, by folder path (no trailing '/').
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileFolds {
    closed: BTreeMap<FileListId, BTreeSet<String>>,
}

impl FileFolds {
    /// The folders collapsed in `list`; none when nothing is.
    pub fn closed(&self, list: FileListId) -> Option<&BTreeSet<String>> {
        self.closed.get(&list).filter(|s| !s.is_empty())
    }

    pub fn is_closed(&self, list: FileListId, folder: &str) -> bool {
        self.closed.get(&list).is_some_and(|s| s.contains(folder))
    }

    /// Collapses `folder` in `list`, or expands it when it was collapsed.
    pub fn toggle(&mut self, list: FileListId, folder: String) {
        let set = self.closed.entry(list).or_default();
        if !set.remove(&folder) {
            set.insert(folder);
        }
    }

    /// Expands every collapsed folder `file` lies in (each ancestor prefix, which also covers a
    /// compacted chain's key), so its row shows. Returns whether anything changed.
    pub fn open_to(&mut self, list: FileListId, file: &str) -> bool {
        let Some(set) = self.closed.get_mut(&list) else { return false };
        let mut changed = false;
        for folder in super::file_tree::ancestors(file) {
            changed |= set.remove(folder);
        }
        changed
    }

    pub fn clear(&mut self) {
        self.closed.clear();
    }
}

/// The tree list `t` was opened from, when the lists show as a tree: None in List mode, while
/// a history is open, and for lists that stay flat (the pull request's, the working-tree
/// comparison).
pub fn list_of(st: &AppState, t: &DiffTarget) -> Option<FileListId> {
    if st.session.history.view.is_some() || st.settings.file_list_mode != FileListMode::Tree {
        return None;
    }
    match t {
        DiffTarget::Unstaged(_) => Some(FileListId::Unstaged),
        DiffTarget::Staged(_) => Some(FileListId::Staged),
        DiffTarget::Commit(oid, _) => {
            if st.session.detail.as_ref().is_some_and(|d| d.oid == *oid) {
                return Some(FileListId::Commit);
            }
            let Selection::Commits(m) = &st.selection else { return None };
            let Some(Ok(stack)) = &m.stacked else { return None };
            stack.iter().any(|c| c.oid == *oid).then_some(FileListId::Stacked(*oid))
        }
        DiffTarget::Between(a, b, _) => {
            if st.session.pull_request.view.as_ref().and_then(|v| v.diff_files(*a, *b)).is_some() {
                return None;
            }
            match &st.selection {
                Selection::Commits(m) if m.compare_ends() == Some((*a, *b)) => Some(FileListId::Compare),
                _ => None,
            }
        }
        DiffTarget::ToWorkdir(..) | DiffTarget::FromWorkdir(..) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(n: u8) -> git2::Oid {
        git2::Oid::from_bytes(&[n; 20]).unwrap()
    }

    #[test]
    fn toggle_closes_then_opens() {
        let mut f = FileFolds::default();
        f.toggle(FileListId::Commit, "src".into());
        assert!(f.is_closed(FileListId::Commit, "src"));
        assert!(f.closed(FileListId::Commit).is_some());
        f.toggle(FileListId::Commit, "src".into());
        assert!(!f.is_closed(FileListId::Commit, "src"));
        assert_eq!(f.closed(FileListId::Commit), None);
    }

    #[test]
    fn lists_keep_their_own_folds() {
        let mut f = FileFolds::default();
        f.toggle(FileListId::Commit, "src".into());
        assert!(!f.is_closed(FileListId::Compare, "src"));
        f.toggle(FileListId::Stacked(oid(1)), "docs".into());
        assert!(f.is_closed(FileListId::Stacked(oid(1)), "docs"));
        assert!(!f.is_closed(FileListId::Stacked(oid(2)), "docs"));
        f.clear();
        assert!(!f.is_closed(FileListId::Commit, "src"));
    }

    #[test]
    fn open_to_opens_every_closed_ancestor() {
        let mut f = FileFolds::default();
        assert!(!f.open_to(FileListId::Unstaged, "a/b/c.txt"), "nothing was closed");
        f.toggle(FileListId::Unstaged, "a".into());
        f.toggle(FileListId::Unstaged, "a/b".into());
        f.toggle(FileListId::Unstaged, "z".into());
        assert!(f.open_to(FileListId::Unstaged, "a/b/c.txt"));
        assert!(!f.is_closed(FileListId::Unstaged, "a") && !f.is_closed(FileListId::Unstaged, "a/b"));
        assert!(f.is_closed(FileListId::Unstaged, "z"));
        assert!(!f.open_to(FileListId::Unstaged, "a/b/c.txt"));
    }
}

//! The lists that only open files (`list_keys::FileList`) as their keys see them: what each
//! row opens, in on-screen order (the tree's, skipping collapsed folders, in Tree mode), and
//! which row is current. Read by `update/file_list_keys.rs`; mirrors what each list's view
//! draws from the same state.

use super::file_folds::FileListId;
use super::list_keys::FileList;
use super::wip_lists::cursor;
use crate::state::history::HistoryKind;
use crate::state::pull_request::Files;
use crate::state::{AppState, Selection};
use glowfly_core::repo::{DiffTarget, FileEntry};
use glowfly_core::settings::FileListMode;

/// What a row opens, as a click on it would.
#[derive(Clone, Debug, PartialEq)]
pub enum Opens {
    /// A diff.
    Diff(DiffTarget),
    /// A conflicted file, in the conflict view.
    Conflict(String),
    /// The folder history's file at this index of the selected commit's files.
    FolderFile(usize),
}

/// A list's rows: what each opens, their on-screen order (indices into `opens`, each with
/// whether it is shown and can be landed on), and the current one.
#[derive(Debug, Default)]
pub struct Listing {
    pub opens: Vec<Opens>,
    pub order: Vec<(usize, bool)>,
    pub current: Option<usize>,
}

impl Listing {
    /// A list of `files` each opening `target(path)`, ordered as the list shows them (`folds`:
    /// the list's folds in Tree mode, None for a list that is never a tree).
    fn diffs(st: &AppState, files: &[FileEntry], folds: Option<FileListId>, target: impl Fn(&str) -> DiffTarget) -> Listing {
        let opens: Vec<Opens> = files.iter().map(|f| Opens::Diff(target(&f.path))).collect();
        let order = match folds {
            Some(list) => tree_order(st, files.iter().map(|f| f.path.as_str()).collect(), list),
            None => (0..files.len()).map(|i| (i, true)).collect(),
        };
        let mut l = Listing { opens, order, current: None };
        l.current = cursor(st).and_then(|t| l.opens.iter().position(|o| *o == Opens::Diff(t.clone())));
        l
    }

    /// `other`'s rows after these, in their own order.
    fn append(&mut self, other: Listing) {
        let base = self.opens.len();
        if self.current.is_none() {
            self.current = other.current.map(|c| c + base);
        }
        self.opens.extend(other.opens);
        self.order.extend(other.order.into_iter().map(|(i, shown)| (i + base, shown)));
    }
}

/// `paths` in on-screen order (indices into it), each with whether it is shown: the tree's
/// order with `list`'s collapsed folders in Tree mode, else the list's own.
pub fn tree_order(st: &AppState, paths: Vec<&str>, list: FileListId) -> Vec<(usize, bool)> {
    if st.settings.file_list_mode != FileListMode::Tree {
        return (0..paths.len()).map(|i| (i, true)).collect();
    }
    let folds = &st.session.tab_view.file_folds;
    super::file_tree::shown_order(&paths, |f| folds.is_closed(list, f))
}

/// `list` as it is shown now; None while it isn't (another selection, still loading, failed).
pub fn listing(st: &AppState, list: FileList) -> Option<Listing> {
    match list {
        FileList::Commit => {
            let Selection::Commit(_) = st.session.selection else { return None };
            let d = st.session.detail.as_ref()?;
            Some(Listing::diffs(st, &d.files, Some(FileListId::Commit), |p| DiffTarget::Commit(d.oid, p.to_owned())))
        }
        FileList::Compare => {
            let Selection::Commits(m) = &st.session.selection else { return None };
            let (from, to) = m.compare_ends()?;
            let c = m.changes.as_ref()?.as_ref().ok()?;
            Some(Listing::diffs(st, &c.files, Some(FileListId::Compare), |p| DiffTarget::Between(from, to, p.to_owned())))
        }
        FileList::Stacked => {
            let Selection::Commits(m) = &st.session.selection else { return None };
            if m.compare_ends().is_some() {
                return None;
            }
            let mut all = Listing::default();
            for c in m.stacked.as_ref()?.as_ref().ok()? {
                all.append(Listing::diffs(st, &c.files, Some(FileListId::Stacked(c.oid)), |p| DiffTarget::Commit(c.oid, p.to_owned())));
            }
            Some(all)
        }
        FileList::Workdir => {
            let Selection::VsWorkdir(w) = &st.session.selection else { return None };
            let c = w.changes.as_ref()?.as_ref().ok()?;
            Some(Listing::diffs(st, &c.files, None, |p| w.target(p.to_owned())))
        }
        FileList::PullRequest => {
            let v = st.session.pull_request.view.as_ref()?;
            let Files::Loaded(c) = &v.files else { return None };
            let (base, tip) = (c.base, v.source_tip()?);
            Some(Listing::diffs(st, &c.comparison.files, None, |p| DiffTarget::Between(base, tip, p.to_owned())))
        }
        FileList::FolderHistory => {
            let view = st.session.history.view.as_ref().filter(|v| v.kind == HistoryKind::Folder)?;
            let n = view.selected_entry()?.files.len();
            let current = (view.file < n).then_some(view.file);
            Some(Listing { opens: (0..n).map(Opens::FolderFile).collect(), order: (0..n).map(|i| (i, true)).collect(), current })
        }
        FileList::Conflicted => {
            let snap = st.snap.as_ref()?;
            let paths: Vec<&str> = snap.unstaged.iter().filter(|f| f.status == '!').map(|f| f.path.as_str()).collect();
            let open = st.session.conflict.view.as_ref().map(|c| c.path.as_str());
            Some(Listing {
                current: open.and_then(|o| paths.iter().position(|p| *p == o)),
                order: (0..paths.len()).map(|i| (i, true)).collect(),
                opens: paths.into_iter().map(|p| Opens::Conflict(p.to_owned())).collect(),
            })
        }
        FileList::Resolved => {
            let snap = st.snap.as_ref()?;
            let rows = st.session.conflict_files.resolved_rows(snap);
            let paths: Vec<&str> = rows.iter().map(|r| r.entry.path.as_str()).collect();
            let opens: Vec<Opens> = paths.iter().map(|p| Opens::Diff(DiffTarget::Staged((*p).to_owned()))).collect();
            // Only a row with a staged change opens anything (its click does nothing else).
            let order = paths.iter().enumerate().map(|(i, p)| (i, snap.staged.iter().any(|f| f.path == *p))).collect();
            let current = cursor(st).and_then(|t| opens.iter().position(|o| *o == Opens::Diff(t.clone())));
            let l = Listing { opens, order, current };
            Some(l)
        }
    }
}

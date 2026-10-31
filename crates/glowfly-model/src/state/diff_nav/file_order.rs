//! The file list a diff was opened from, in on-screen order: where the list lives (`ListSource`),
//! its files as diff targets with their status letters, the open file's place and neighbours in it
//! (`FileOrder`), and a key that tells cheaply when the list may have changed (`ListKey`).

use crate::state::file_tree;
use crate::state::history::HistoryKind;
use crate::state::selection_wip::WorkdirCompare;
use crate::state::{AppState, Selection};
use git2::Oid;
use glowfly_core::repo::{DiffTarget, FileEntry, StackedCommit};
use glowfly_core::settings::FileListMode;

/// The lists a diff can be opened from, as the slices they read.
enum ListSource<'a> {
    /// A folder history's selected entry.
    History(Oid, &'a [FileEntry]),
    /// The Unstaged then the Staged Files list.
    Wip(&'a [FileEntry], &'a [FileEntry]),
    /// The commit details list.
    Commit(Oid, &'a [FileEntry]),
    /// A stacked list's files across its commits.
    Stacked(&'a [StackedCommit]),
    /// The open pull request's list (never a tree).
    PullRequest(Oid, Oid, &'a [FileEntry]),
    /// A comparison of two commits.
    Compare(Oid, Oid, &'a [FileEntry]),
    /// A comparison with the working tree, in its current direction (never a tree).
    Workdir(&'a WorkdirCompare, &'a [FileEntry]),
}

/// Where a slice's items are and how many there are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Slice(usize, usize);

fn slice<T>(s: &[T]) -> Slice {
    Slice(s.as_ptr() as usize, s.len())
}

/// What the file order of a diff was built from: the target, the list mode, and the lists it read
/// by address and length (and the commit details, a file history's fallback for the status).
/// Lists are replaced whole when they are read again, the new one allocated while the old one is
/// still alive, so a reload always shows here as a new address or length.
#[derive(Clone, Debug, PartialEq)]
pub struct ListKey {
    from: DiffTarget,
    tree: bool,
    source: Option<(u8, [Slice; 2])>,
    detail: Option<(Oid, Slice)>,
}

impl ListSource<'_> {
    fn key(&self) -> (u8, [Slice; 2]) {
        let none = Slice::default();
        match self {
            ListSource::History(_, f) => (0, [slice(f), none]),
            ListSource::Wip(u, s) => (1, [slice(u), slice(s)]),
            ListSource::Commit(_, f) => (2, [slice(f), none]),
            ListSource::Stacked(s) => (3, [slice(s), none]),
            ListSource::PullRequest(_, _, f) => (4, [slice(f), none]),
            ListSource::Compare(_, _, f) => (5, [slice(f), none]),
            ListSource::Workdir(_, f) => (6, [slice(f), none]),
        }
    }

    fn entries(&self, st: &AppState) -> Vec<(DiffTarget, char)> {
        let of = |files: &[FileEntry], make: &dyn Fn(String) -> DiffTarget| -> Vec<(DiffTarget, char)> {
            files.iter().map(|f| (make(f.path.clone()), f.status)).collect()
        };
        // The lists drawn as a tree walk their files in tree order.
        let shown = |files: &[FileEntry], make: &dyn Fn(String) -> DiffTarget| tree_ordered(st, of(files, make));
        match *self {
            ListSource::History(oid, files) => of(files, &|p| DiffTarget::Commit(oid, p)),
            ListSource::Wip(unstaged, staged) => {
                let unstaged = unstaged.iter().filter(|f| f.status != '!').map(|f| (DiffTarget::Unstaged(f.path.clone()), f.status));
                let staged = staged.iter().map(|f| (DiffTarget::Staged(f.path.clone()), f.status));
                tree_ordered(st, unstaged.collect()).into_iter().chain(tree_ordered(st, staged.collect())).collect()
            }
            ListSource::Commit(oid, files) => shown(files, &|p| DiffTarget::Commit(oid, p)),
            ListSource::Stacked(stack) => stack.iter().flat_map(|c| shown(&c.files, &|p| DiffTarget::Commit(c.oid, p))).collect(),
            ListSource::PullRequest(a, b, files) => of(files, &|p| DiffTarget::Between(a, b, p)),
            ListSource::Compare(a, b, files) => shown(files, &|p| DiffTarget::Between(a, b, p)),
            ListSource::Workdir(w, files) => of(files, &|p| w.target(p)),
        }
    }
}

/// The list `from` was opened from: the Unstaged then the Staged Files list (conflicted entries
/// open the conflict view, so they are left out), the commit details list, the open pull
/// request's list, the comparison list, a stacked list's files across its commits, or in a history
/// the selected folder entry's files. `None` in a file history, which shows one file, or when the
/// list isn't the one on screen.
fn list_source<'a>(st: &'a AppState, from: &DiffTarget) -> Option<ListSource<'a>> {
    if let Some(h) = &st.session.history.view {
        let DiffTarget::Commit(oid, _) = from else { return None };
        let e = h.selected_entry().filter(|e| h.kind == HistoryKind::Folder && e.oid == *oid)?;
        return Some(ListSource::History(*oid, &e.files));
    }
    match from {
        DiffTarget::Unstaged(_) | DiffTarget::Staged(_) => {
            let snap = st.snap.as_ref()?;
            Some(ListSource::Wip(&snap.unstaged, &snap.staged))
        }
        DiffTarget::Commit(oid, _) => {
            if let Some(d) = st.session.detail.as_ref().filter(|d| d.oid == *oid) {
                return Some(ListSource::Commit(*oid, &d.files));
            }
            let Selection::Commits(m) = &st.session.selection else { return None };
            let Some(Ok(stack)) = &m.stacked else { return None };
            stack.iter().any(|c| c.oid == *oid).then_some(ListSource::Stacked(stack))
        }
        DiffTarget::Between(a, b, _) => {
            if let Some(files) = st.session.pull_request.view.as_ref().and_then(|v| v.diff_files(*a, *b)) {
                return Some(ListSource::PullRequest(*a, *b, files));
            }
            match &st.session.selection {
                Selection::Commits(m) if m.compare_ends() == Some((*a, *b)) => match &m.changes {
                    Some(Ok(c)) => Some(ListSource::Compare(*a, *b, &c.files)),
                    _ => None,
                },
                _ => None,
            }
        }
        // Only the list of the comparison's current direction: a ToWorkdir target is never
        // walked through a swapped list, or the reverse.
        DiffTarget::ToWorkdir(o, _) | DiffTarget::FromWorkdir(o, _) => match &st.session.selection {
            Selection::VsWorkdir(w) if w.oid == *o && w.swapped == matches!(from, DiffTarget::FromWorkdir(..)) => match &w.changes {
                Some(Ok(c)) => Some(ListSource::Workdir(w, &c.files)),
                _ => None,
            },
            _ => None,
        },
    }
}

/// `entries` (one list's files) in tree order while the lists show as a tree, else unchanged.
fn tree_ordered(st: &AppState, entries: Vec<(DiffTarget, char)>) -> Vec<(DiffTarget, char)> {
    if st.settings.file_list_mode != FileListMode::Tree {
        return entries;
    }
    let paths: Vec<&str> = entries.iter().map(|(t, _)| t.path()).collect();
    let order = file_tree::file_order(&paths);
    let mut slots: Vec<Option<(DiffTarget, char)>> = entries.into_iter().map(Some).collect();
    order.into_iter().filter_map(|i| slots[i].take()).collect()
}

/// The file list `from` was opened from (`list_source`), in on-screen order, with each file's
/// status letter.
pub fn source_entries(st: &AppState, from: &DiffTarget) -> Option<Vec<(DiffTarget, char)>> {
    Some(list_source(st, from)?.entries(st))
}

/// What `file_order` for `from` is built from; equal keys give the same order.
pub fn key(st: &AppState, from: &DiffTarget) -> ListKey {
    ListKey {
        from: from.clone(),
        tree: st.settings.file_list_mode == FileListMode::Tree,
        source: list_source(st, from).map(|s| s.key()),
        detail: st.session.detail.as_ref().map(|d| (d.oid, slice(&d.files))),
    }
}

/// A diff's place in the list it was opened from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileOrder {
    /// The files before and after it; nothing wraps.
    pub prev: Option<DiffTarget>,
    pub next: Option<DiffTarget>,
    /// Its index and the list's length, when it is listed.
    pub place: Option<(usize, usize)>,
    /// Its status letter (M, A, D, R, T, ?), when it is listed.
    pub status: Option<char>,
}

impl FileOrder {
    /// The neighbour a step of `dir` (1 next, -1 previous) goes to.
    pub fn neighbour(&self, dir: isize) -> Option<&DiffTarget> {
        if dir > 0 {
            self.next.as_ref()
        } else if dir < 0 {
            self.prev.as_ref()
        } else {
            None
        }
    }
}

/// `target`'s place in the list it came from, in one walk of the list.
pub fn file_order(st: &AppState, target: &DiffTarget) -> FileOrder {
    let mut order = FileOrder::default();
    if let Some(files) = source_entries(st, target)
        && let Some(i) = files.iter().position(|(t, _)| t == target)
    {
        order.status = Some(files[i].1);
        order.place = Some((i, files.len()));
        order.next = files.get(i + 1).map(|(t, _)| t.clone());
        order.prev = i.checked_sub(1).map(|j| files[j].0.clone());
    }
    order.status = order.status.or_else(|| detail_status(st, target));
    order
}

/// A file history's commit diff isn't in a list but its detail may still name it.
fn detail_status(st: &AppState, target: &DiffTarget) -> Option<char> {
    let DiffTarget::Commit(oid, path) = target else { return None };
    st.session.detail.as_ref().filter(|d| d.oid == *oid)?.files.iter().find(|f| f.path == *path).map(|f| f.status)
}

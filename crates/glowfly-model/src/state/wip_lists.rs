//! The Unstaged and Staged Files lists as their keys see them: their files in on-screen order
//! (the tree's, skipping collapsed folders, in Tree mode), the file the keys act on, and the
//! files picked with Shift or Ctrl/Cmd, gaps allowed (`WipPicks`, read as `Picked`; `range`
//! for a Shift run, `prune` after a status read). Read by `update/wip_list.rs`,
//! `update/wip_picks.rs` and the lists' view.

use super::file_folds::FileListId;
use super::list_keys::{Place, place_in};
use crate::state::AppState;
use glowfly_core::repo::{DiffTarget, FileEntry, Snapshot};
use std::collections::{BTreeSet, HashSet};

/// The files picked in one list with Shift or Ctrl/Cmd (keys or clicks), gaps allowed;
/// `anchor` is where the next Shift range starts. It holds only while the file the keys act on
/// is one of them, so a plain click or move elsewhere ends it. View state, changed only by
/// `update`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WipPicks {
    pub staged: bool,
    pub anchor: String,
    pub paths: BTreeSet<String>,
}

/// How a click picks files: Shift (`Range`, from the anchor) or Ctrl/Cmd (`Toggle`, one file).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickHow {
    Range,
    Toggle,
}

/// The picked files of one list: in on-screen order and as a set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picked {
    pub paths: Vec<String>,
    pub set: HashSet<String>,
}

/// The open file staged or unstaged as a whole (`from`) and the file to open once that worked
/// (`to`: the nearest one not staged or unstaged with it, after it first).
#[derive(Clone, Debug, PartialEq)]
pub struct MoveOn {
    pub from: DiffTarget,
    pub to: Option<DiffTarget>,
}

/// The Unstaged (`staged: false`) or Staged Files list as it is shown: Unstaged Files leaves
/// the conflicted entries to their own list.
pub fn files(snap: &Snapshot, staged: bool) -> Vec<&FileEntry> {
    if staged { snap.staged.iter().collect() } else { snap.unstaged.iter().filter(|f| f.status != '!').collect() }
}

/// The diff a row of that list opens.
pub fn target(staged: bool, path: &str) -> DiffTarget {
    if staged { DiffTarget::Staged(path.to_owned()) } else { DiffTarget::Unstaged(path.to_owned()) }
}

/// The file the keys act on: the one still opening, else the one on screen.
pub fn cursor(st: &AppState) -> Option<&DiffTarget> {
    st.cues.diff_opening.as_ref().or(st.session.diff.view.as_ref().map(|(t, _)| t))
}

/// The path of `t` when it is a diff of that list's kind.
pub fn path_in(staged: bool, t: &DiffTarget) -> Option<&str> {
    match t {
        DiffTarget::Staged(p) if staged => Some(p),
        DiffTarget::Unstaged(p) if !staged => Some(p),
        _ => None,
    }
}

/// `files` in on-screen order (indices into it), each with whether it is shown: the tree's
/// order with its collapsed folders in Tree mode, else the list's own.
pub fn order(st: &AppState, files: &[&FileEntry], staged: bool) -> Vec<(usize, bool)> {
    let list = if staged { FileListId::Staged } else { FileListId::Unstaged };
    super::file_lists::tree_order(st, files.iter().map(|f| f.path.as_str()).collect(), list)
}

/// The list's visible files in order, as indices into `files`.
pub fn visible(order: &[(usize, bool)]) -> Vec<usize> {
    order.iter().filter(|(_, shown)| *shown).map(|(i, _)| *i).collect()
}

/// Where the file at `path` sits among the list's visible files.
pub fn place(files: &[&FileEntry], order: &[(usize, bool)], path: Option<&str>) -> Place {
    place_in(order, path.and_then(|p| files.iter().position(|f| f.path == p)))
}

/// The picks of that list while they hold (the file the keys act on is one of them).
pub fn picks(st: &AppState, staged: bool) -> Option<&WipPicks> {
    let p = st.cues.wip_picks.as_ref().filter(|p| p.staged == staged)?;
    cursor(st).and_then(|t| path_in(staged, t)).filter(|c| p.paths.contains(*c)).map(|_| p)
}

/// The files picked in that list, in on-screen order: only the visible ones, so a file that
/// left the list or sits in a collapsed folder doesn't count. Empty without picks.
pub fn picked_paths(st: &AppState, staged: bool) -> Vec<String> {
    let (Some(p), Some(snap)) = (picks(st, staged), st.snap.as_ref()) else { return Vec::new() };
    let files = files(snap, staged);
    let order = order(st, &files, staged);
    visible(&order).into_iter().map(|i| &files[i].path).filter(|f| p.paths.contains(*f)).cloned().collect()
}

/// The picked files of that list, for the rows to draw selected and the actions to take; None
/// unless there are several.
pub fn picked(st: &AppState, staged: bool) -> Option<Picked> {
    let paths = picked_paths(st, staged);
    (paths.len() > 1).then(|| Picked { set: paths.iter().cloned().collect(), paths })
}

/// The visible files of that list from `from` to `to`, both included (either may come first);
/// just `to` when `from` isn't visible.
pub fn range(st: &AppState, staged: bool, from: &str, to: &str) -> Vec<String> {
    let Some(snap) = st.snap.as_ref() else { return vec![to.to_owned()] };
    let files = files(snap, staged);
    let order = order(st, &files, staged);
    let (Place::At(a), Place::At(b)) = (place(&files, &order, Some(from)), place(&files, &order, Some(to))) else {
        return vec![to.to_owned()];
    };
    let visible = visible(&order);
    visible[a.min(b)..=a.max(b)].iter().map(|&i| files[i].path.clone()).collect()
}

/// Forgets the picks no longer in their list (after a status read), so a file that comes back
/// doesn't rejoin; no picks are left when none is.
pub fn prune(st: &mut AppState) {
    let Some(snap) = st.snap.as_ref() else { return };
    let Some(p) = st.cues.wip_picks.as_mut() else { return };
    let listed: HashSet<&str> = files(snap, p.staged).into_iter().map(|f| f.path.as_str()).collect();
    p.paths.retain(|f| listed.contains(f.as_str()));
    if p.paths.is_empty() {
        st.cues.wip_picks = None;
    }
}

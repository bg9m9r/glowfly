//! The Unstaged and Staged Files lists as their keys see them: their files in on-screen order
//! (the tree's, skipping collapsed folders, in Tree mode), the file the keys act on, and the
//! files a Shift move selected (`WipSpan`). Read by `update/wip_list.rs` and the lists' view.

use super::file_folds::FileListId;
use super::list_keys::{Place, place_in};
use crate::state::AppState;
use glowfly_core::repo::{DiffTarget, FileEntry, Snapshot};
use std::collections::HashSet;

/// The files a Shift move selected in one list: every visible file from `anchor` to `head`.
/// It holds only while `head` is still the file the keys act on, so a click or a plain move
/// elsewhere ends it. View state, changed only by `update`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WipSpan {
    pub staged: bool,
    pub anchor: String,
    pub head: String,
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
    st.view.diff_opening.as_ref().or(st.session.diff.view.as_ref().map(|(t, _)| t))
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

/// The span of that list while it holds (its head is the file the keys act on).
pub fn span(st: &AppState, staged: bool) -> Option<&WipSpan> {
    let s = st.view.wip_span.as_ref().filter(|s| s.staged == staged)?;
    cursor(st).and_then(|t| path_in(staged, t)).filter(|p| *p == s.head).map(|_| s)
}

/// The files the span of that list selects, in on-screen order: the visible ones from its
/// anchor to its head. Empty without a span, or when its anchor has left the list.
pub fn span_paths(st: &AppState, staged: bool) -> Vec<String> {
    let (Some(s), Some(snap)) = (span(st, staged), st.snap.as_ref()) else { return Vec::new() };
    let files = files(snap, staged);
    let order = order(st, &files, staged);
    let (Place::At(a), Place::At(h)) = (place(&files, &order, Some(&s.anchor)), place(&files, &order, Some(&s.head))) else {
        return Vec::new();
    };
    let visible = visible(&order);
    visible[a.min(h)..=a.max(h)].iter().map(|&i| files[i].path.clone()).collect()
}

/// The span's files as a set, for the rows to draw selected; None unless it selects several.
pub fn span_set(st: &AppState, staged: bool) -> Option<HashSet<String>> {
    let paths = span_paths(st, staged);
    (paths.len() > 1).then(|| paths.into_iter().collect())
}

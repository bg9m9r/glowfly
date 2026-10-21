//! The WIP lists' keyboard: Up/Down between the Unstaged or Staged Files, moving on to the
//! neighbouring file after S or U staged or unstaged the open one, and asking again for a file
//! still opening when a refresh would reload the old one.

use crate::effect::Effect;
use crate::state::AppState;
use glowfly_core::repo::{DiffTarget, FileEntry, Snapshot};

/// The Unstaged (`staged: false`) or Staged Files list as it is shown: Unstaged Files leaves
/// the conflicted entries to their own list.
fn files(snap: &Snapshot, staged: bool) -> Vec<&FileEntry> {
    if staged { snap.staged.iter().collect() } else { snap.unstaged.iter().filter(|f| f.status != '!').collect() }
}

/// The diff a row of that list opens.
fn target(staged: bool, path: &str) -> DiffTarget {
    if staged { DiffTarget::Staged(path.to_owned()) } else { DiffTarget::Unstaged(path.to_owned()) }
}

/// The file the keys act on: the one still opening, else the one on screen.
pub(super) fn cursor(st: &AppState) -> Option<&DiffTarget> {
    st.view.diff_opening.as_ref().or(st.diff.as_ref().map(|(t, _)| t))
}

/// Where `t` sits in `files`, when it is a diff of that list's kind.
fn index_in(files: &[&FileEntry], staged: bool, t: &DiffTarget) -> Option<usize> {
    let path = match t {
        DiffTarget::Staged(p) if staged => p,
        DiffTarget::Unstaged(p) if !staged => p,
        _ => return None,
    };
    files.iter().position(|f| f.path == *path)
}

/// The file after `path` in its list, else the one before it, else none.
pub(super) fn neighbour(snap: &Snapshot, staged: bool, path: &str) -> Option<DiffTarget> {
    let files = files(snap, staged);
    let i = files.iter().position(|f| f.path == path)?;
    let next = files.get(i + 1).or_else(|| i.checked_sub(1).and_then(|j| files.get(j)))?;
    Some(target(staged, &next.path))
}

/// Opens `t` as a click on its row would, and has its list scroll it into view once.
fn open(st: &mut AppState, t: DiffTarget, fx: &mut Vec<Effect>) {
    super::repo::open_file_diff(st, t.clone(), fx);
    // After the open, which clears it.
    st.view.file_reveal = Some((t, st.diff_gen));
}

/// Up (`down: false`) or Down in the focused list: the file above or below the one the keys
/// act on, or the list's first file when that one isn't in the list. Nothing at either end.
pub(super) fn step(st: &mut AppState, staged: bool, down: bool, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    let files = files(snap, staged);
    if files.is_empty() {
        return;
    }
    let at = cursor(st).and_then(|t| index_in(&files, staged, t));
    let next = match at {
        Some(i) if down => i + 1,
        Some(i) => match i.checked_sub(1) {
            Some(j) => j,
            None => return,
        },
        None => 0,
    };
    let Some(f) = files.get(next) else { return };
    let t = target(staged, &f.path);
    open(st, t, fx);
}

/// S or U sent a whole-file stage or unstage of `t`.
pub(super) fn stage_sent(st: &mut AppState, t: DiffTarget) {
    st.view.stage_move_on = Some(t);
}

/// A working-tree operation finished (`ok`): when it was S or U on the file the keys still act
/// on, opens the file that was next to it in its list (the status read hasn't come back, so
/// the list is as it stood).
pub(super) fn op_done(st: &mut AppState, ok: bool, fx: &mut Vec<Effect>) {
    let Some(left) = st.view.stage_move_on.take() else { return };
    if !ok || cursor(st) != Some(&left) {
        return;
    }
    let staged = matches!(left, DiffTarget::Staged(_));
    let next = st.snap.as_ref().and_then(|s| neighbour(s, staged, left.path()));
    if let Some(t) = next {
        open(st, t, fx);
    }
}

/// A refresh would reload the diff on screen while another file is still opening: asks for that
/// file again instead, while its list still holds it. Returns whether it did.
pub(super) fn renew_open(st: &mut AppState, fx: &mut Vec<Effect>) -> bool {
    let Some(t) = st.view.diff_opening.clone() else { return false };
    if !still_listed(st, &t) {
        st.view.diff_opening = None;
        return false;
    }
    let reveal = st.view.file_reveal.take();
    super::repo::open_diff(st, t, fx);
    st.view.file_reveal = reveal;
    true
}

/// Whether a working-tree target is still in its list; any other target always is.
fn still_listed(st: &AppState, t: &DiffTarget) -> bool {
    let Some(snap) = st.snap.as_ref() else { return false };
    match t {
        DiffTarget::Unstaged(p) => snap.unstaged.iter().any(|f| f.path == *p && f.status != '!'),
        DiffTarget::Staged(p) => snap.staged.iter().any(|f| f.path == *p),
        _ => true,
    }
}

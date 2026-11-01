//! The WIP lists' moves: opening the file a key lands on in the Unstaged or Staged Files,
//! moving on to the nearest file after the open one was staged or unstaged as a whole (by its
//! key, button or menu, alone or with the selected files), asking again for a file still
//! opening when a refresh would reload the old one, and following a new file unstaged to its
//! untracked view when its staged diff empties. In Tree mode the keys follow the tree's
//! on-screen order and skip the files of collapsed folders (`state/wip_lists.rs`). The keys
//! themselves are in `wip_keys.rs`.

use crate::effect::Effect;
use crate::state::AppState;
use crate::state::list_keys::ListMove;
use crate::state::wip_lists::{MoveOn, files, order, path_in, place, target, visible};
use glowfly_core::repo::DiffTarget;

pub(super) use crate::state::wip_lists::cursor;

/// Opens `t` as a click on its row would, and has its list scroll it into view once.
pub(super) fn open(st: &mut AppState, t: DiffTarget, fx: &mut Vec<Effect>) {
    super::repo::open_file_diff(st, t.clone(), fx);
    // After the open, which clears it.
    st.view.file_reveal = Some((t, st.session.diff.generation));
}

/// The file `m` lands on in that list from the file the keys act on (from the list's first
/// file when that one isn't in it). None at either end or in an empty list.
pub(super) fn landing(st: &AppState, staged: bool, m: ListMove) -> Option<DiffTarget> {
    let snap = st.snap.as_ref()?;
    let files = files(snap, staged);
    let order = order(st, &files, staged);
    let at = place(&files, &order, cursor(st).and_then(|t| path_in(staged, t)));
    let visible = visible(&order);
    let to = m.target(visible.len(), at)?;
    Some(target(staged, &files[visible[to]].path))
}

/// Opens the file `m` lands on in that list, if any.
pub(super) fn step(st: &mut AppState, staged: bool, m: ListMove, fx: &mut Vec<Effect>) {
    if let Some(t) = landing(st, staged, m) {
        open(st, t, fx);
    }
}

/// The visible file of that list nearest to `path` that isn't one of `leaving`: after it
/// first, then before it.
fn neighbour(st: &AppState, staged: bool, path: &str, leaving: &[String]) -> Option<DiffTarget> {
    let snap = st.snap.as_ref()?;
    let files = files(snap, staged);
    let order = order(st, &files, staged);
    let at = order.iter().position(|&(i, _)| files[i].path == path)?;
    let stays = |&&(i, shown): &&(usize, bool)| shown && !leaving.iter().any(|p| *p == files[i].path);
    let next = order[at + 1..].iter().find(stays).or_else(|| order[..at].iter().rev().find(stays))?;
    Some(target(staged, &files[next.0].path))
}

/// A whole-file stage (`staged: false`, from Unstaged Files) or unstage of `paths` was sent:
/// when the file the keys act on is one of them, its operation's success opens the nearest file
/// left in its list.
pub(super) fn stage_sent(st: &mut AppState, staged: bool, paths: &[String]) {
    if st.workdir().is_none() {
        return;
    }
    let acting = |t: &&DiffTarget| path_in(staged, t).is_some_and(|p| paths.iter().any(|q| q == p));
    let Some(from) = cursor(st).filter(acting).cloned() else { return };
    let to = neighbour(st, staged, from.path(), paths);
    st.view.stage_move_on = Some(MoveOn { from, to });
    st.view.wip_span = None;
}

/// A working-tree operation finished (`ok`): when it staged or unstaged the file the keys still
/// act on, opens the file `stage_sent` picked.
pub(super) fn op_done(st: &mut AppState, ok: bool, fx: &mut Vec<Effect>) {
    let Some(MoveOn { from, to }) = st.view.stage_move_on.take() else { return };
    if !ok || cursor(st) != Some(&from) {
        return;
    }
    if let Some(t) = to {
        open(st, t, fx);
    }
}

/// A file of either list was opened: going to that list from the other one lands there.
pub(super) fn opened(st: &mut AppState, t: &DiffTarget) {
    let (staged, path) = match t {
        DiffTarget::Unstaged(p) => (false, p),
        DiffTarget::Staged(p) => (true, p),
        _ => return,
    };
    st.view.wip_place[usize::from(staged)] = Some(path.clone());
}

/// A reload of the diff on screen found no changes left: closes it, unless it was a new file's
/// staged diff and the file is untracked now (every line of it unstaged), which it follows to
/// its untracked view.
pub(super) fn emptied(st: &mut AppState, target: DiffTarget, fx: &mut Vec<Effect>) {
    st.session.diff.view = None;
    st.view.clear_diff_selection();
    st.session.tab_view.diff_expanded.clear();
    st.view.diff_open_hunk = None;
    let untracked = |p: &str| st.snap.as_ref().is_some_and(|s| s.unstaged.iter().any(|f| f.path == p && f.status == '?'));
    if let DiffTarget::Staged(p) = target
        && untracked(&p)
    {
        open(st, DiffTarget::Unstaged(p), fx);
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
pub(super) fn still_listed(st: &AppState, t: &DiffTarget) -> bool {
    let Some(snap) = st.snap.as_ref() else { return false };
    match t {
        DiffTarget::Unstaged(p) => files(snap, false).iter().any(|f| f.path == *p),
        DiffTarget::Staged(p) => files(snap, true).iter().any(|f| f.path == *p),
        _ => true,
    }
}

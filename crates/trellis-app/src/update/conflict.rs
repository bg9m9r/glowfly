//! The conflict view: opening a conflicted file, stepping between chunks, saving (write and
//! stage), saving with markers left (write only) or resolving with a whole side. After Trellis
//! resolves the open file the view steps to the next conflicted one. Picking sides and lines
//! and editing the output are in update/conflict_edit.rs; Mark resolved and Mark conflicted in
//! update/conflict_files.rs. Undoing a saved resolution opens (or reloads) the view on the file
//! again (`reopen_after_undo`).

use crate::effect::{Effect, LocalOp, UndoResult};
use crate::msg::ConflictMsg;
use crate::state::AppState;
use crate::state::conflict::ConflictView;
use crate::state::conflict_files::next_conflicted;
use crate::state::tool_compare::merging;
use std::time::Instant;
use trellis_core::repo::Choice;
use trellis_core::undo::Action;

#[path = "conflict_edit.rs"]
mod edit;

pub(super) fn handle(st: &mut AppState, msg: ConflictMsg, fx: &mut Vec<Effect>) {
    match msg {
        ConflictMsg::OpenConflict(path) => open(st, path, fx),
        ConflictMsg::ConflictLoaded { path, generation, result, saved } => {
            if generation != st.conflict_gen || st.conflict.as_ref().is_none_or(|c| c.path != path) {
                return; // superseded: closed, reopened or another file opened
            }
            match result {
                Ok(f) => {
                    if let Some(c) = st.conflict.as_mut() {
                        c.set_file_saved(f, saved);
                        // The first chunk still to pick (automatic picks come before it).
                        st.view.conflict_scroll_to = Some(c.current);
                    }
                }
                Err(e) => {
                    st.log(false, e);
                    close(st);
                }
            }
        }
        ConflictMsg::CloseConflict => close(st),
        ConflictMsg::ConflictToggle { chunk, side } => edit::toggle_side(st, chunk, side),
        ConflictMsg::ConflictToggleLine { chunk, side, line } => edit::toggle_line(st, chunk, side, line),
        ConflictMsg::ConflictTakeAll(side) => edit::set_all(st, Choice::only(side)),
        ConflictMsg::ConflictReset => edit::set_all(st, Choice::Unresolved),
        ConflictMsg::ConflictEditOutput(on) => edit::set_edit(st, on),
        ConflictMsg::ConflictOutputEdited => edit::edited(st),
        ConflictMsg::ConflictDiscardEdit(k) => edit::discard_edit(st, k),
        ConflictMsg::ConflictDiscardCleanEdits => edit::discard_cleans(st),
        ConflictMsg::ConflictFold { seg, open } => {
            if let Some(c) = st.conflict.as_mut() {
                if open {
                    c.opened.insert(seg);
                } else {
                    c.opened.remove(&seg);
                }
            }
        }
        ConflictMsg::ConflictStep(d) => {
            let Some(c) = st.conflict.as_mut() else { return };
            let n = c.picks.len();
            if n == 0 {
                return;
            }
            let next = (c.current as isize + d).clamp(0, n as isize - 1) as usize;
            if next != c.current {
                c.current = next;
                st.view.conflict_scroll_to = Some(next);
            }
        }
        ConflictMsg::ConflictSelect(i) => {
            if let Some(c) = st.conflict.as_mut()
                && i < c.picks.len()
            {
                c.current = i;
            }
        }
        ConflictMsg::SaveConflict => {
            let path = st.conflict.as_ref().map(|c| c.path.clone()).unwrap_or_default();
            if in_tool(st, &path) {
                return;
            }
            let Some(c) = st.conflict.as_mut() else { return };
            // The button is disabled until every chunk has a pick.
            if !c.can_save() {
                return;
            }
            let (Some(contents), Some(f)) = (c.output(), c.file.as_ref()) else { return };
            let op = LocalOp::ResolveConflict { path: c.path.clone(), contents, expected: f.ids.clone() };
            c.saving = true;
            c.step_on_resolve = true;
            super::ops::local(st, op, fx);
        }
        ConflictMsg::SaveConflictKeepMarkers => {
            let path = st.conflict.as_ref().map(|c| c.path.clone()).unwrap_or_default();
            if in_tool(st, &path) {
                return;
            }
            let Some(c) = st.conflict.as_mut() else { return };
            // The button is disabled until something is resolved and something isn't.
            if !c.can_save_partial() {
                return;
            }
            let (Some(contents), Some(f)) = (c.partial_output(), c.file.as_ref()) else { return };
            let op = LocalOp::SavePartialResolution { path: c.path.clone(), contents, expected: f.ids.clone() };
            let n = c.unresolved();
            let note = format!("Saved {} with {n} conflict{} left (still conflicted)", c.path, if n == 1 { "" } else { "s" });
            c.pending_save = Some((note, c.picks.clone(), c.cleans.clone()));
            c.saving = true;
            super::ops::local(st, op, fx);
        }
        ConflictMsg::ResolveWithSide { path, side } => {
            if in_tool(st, &path) {
                return;
            }
            let Some(c) = st.conflict.as_mut().filter(|c| c.path == path && !c.saving) else { return };
            let Some(f) = &c.file else { return };
            let op = LocalOp::ResolveWithSide { path, side, expected: f.ids.clone() };
            c.saving = true;
            c.step_on_resolve = true;
            super::ops::local(st, op, fx);
        }
        ConflictMsg::MarkResolved(paths) => super::conflict_files::mark_resolved(st, paths, fx),
        ConflictMsg::MarkResolvedConfirmed(paths) => super::conflict_files::confirmed(st, paths, fx),
        ConflictMsg::MarkConflicted(paths) => super::conflict_files::mark_conflicted(st, paths, fx),
        ConflictMsg::ConflictFilesRead { purpose, paths, result } => super::conflict_files::read_done(st, purpose, paths, result, fx),
    }
}

/// Opens the conflict view on `path`, closing the views it replaces.
pub(super) fn open(st: &mut AppState, path: String, fx: &mut Vec<Effect>) {
    if st.workdir().is_none() {
        return;
    }
    super::repo::close_diff(st);
    super::history::close(st);
    super::blame::close(st);
    super::pull_request::close(st);
    super::issue::close(st);
    st.conflict = Some(ConflictView::loading(path));
    request_load(st, fx);
}

/// The file to show again after an undo job finished (`ok`): an undone resolution or save with
/// markers opens it (or reloads it when open), a redone save with markers reloads it only when
/// open (`false`). A redone resolution needs nothing: the next status read closes the view.
pub(super) fn after_undo(undo: &Option<UndoResult>, ok: bool) -> Option<(String, bool)> {
    match undo {
        _ if !ok => None,
        Some(UndoResult::Done { redo: false, before: Action::Resolve { path, .. }, .. }) => Some((path.clone(), true)),
        Some(UndoResult::Done { redo: true, before: Action::Resolve { path, staged: None, .. }, .. }) => Some((path.clone(), false)),
        _ => None,
    }
}

/// Shows `path` again after an undo changed it: reloaded when the view is open on it (picks
/// saved with markers come back), and with `open_closed` opened when the view is closed or holds
/// nothing unsaved. Unsaved choices in another file are never thrown away; the log says where to
/// find `path` instead.
pub(super) fn reopen_after_undo(st: &mut AppState, path: String, open_closed: bool, fx: &mut Vec<Effect>) {
    match &st.conflict {
        Some(c) if c.path == path => open(st, path, fx),
        Some(c) if c.user_choices() > 0 => {
            if open_closed {
                st.log(true, format!("{path} is conflicted again: open it from Conflicted Files to resolve it."));
            }
        }
        _ if open_closed => open(st, path, fx),
        _ => {}
    }
}

/// Whether the merge tool has `path` open, said in the log: writing the file now would race
/// with the tool's save (the buttons are disabled; a confirm dialog can still answer).
pub(super) fn in_tool(st: &mut AppState, path: &str) -> bool {
    let busy = merging(st, path);
    if busy {
        st.log(false, format!("{path} is open in the merge tool: save and close it there first."));
    }
    busy
}

/// Requests the open conflict's file; older results are ignored from now on.
fn request_load(st: &mut AppState, fx: &mut Vec<Effect>) {
    st.conflict_gen += 1;
    if let (Some(workdir), Some(c)) = (st.workdir(), &st.conflict) {
        fx.push(Effect::LoadConflict { workdir, path: c.path.clone(), generation: st.conflict_gen });
    }
}

/// Closes the view; a load still in flight must not reopen it.
pub(super) fn close(st: &mut AppState) {
    st.conflict = None;
    st.conflict_gen += 1;
}

/// After a tab switch: a conflict whose load was dropped while parked is requested again.
pub(super) fn reload_pending(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.conflict.as_ref().is_some_and(|c| c.file.is_none()) {
        request_load(st, fx);
    }
}

/// The working-tree status changed: the conflict lists are read again, and once the open file
/// is no longer conflicted the view moves on. Resolved by Trellis (saved, a whole side, marked
/// resolved) it opens the next conflicted file; otherwise (outside the app) it closes. Picks
/// are otherwise kept.
pub(super) fn status_changed(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    super::conflict_files::want(st, fx);
    let (Some(c), Some(snap)) = (&st.conflict, &st.snap) else { return };
    let conflicted: Vec<&str> = snap.unstaged.iter().filter(|f| f.status == '!').map(|f| f.path.as_str()).collect();
    if conflicted.contains(&c.path.as_str()) {
        return;
    }
    let n = conflicted.len();
    if c.step_on_resolve
        && let Some(next) = next_conflicted(snap, &c.path)
    {
        let text = format!("Resolved {}. Opened {next} ({n} conflicted file{} left)", c.path, if n == 1 { "" } else { "s" });
        open(st, next, fx);
        st.toast(true, text, now);
        return;
    }
    let mut text = format!("Resolved {}", c.path);
    match conflicted.len() {
        0 => {}
        1 => text.push_str(" (1 conflicted file left)"),
        n => text.push_str(&format!(" ({n} conflicted files left)")),
    }
    close(st);
    st.toast(true, text, now);
}

//! The conflict view: opening a conflicted file, picking sides per chunk, stepping between
//! chunks, and saving (write and stage) or resolving with a whole side.

use crate::effect::{Effect, LocalOp};
use crate::msg::ConflictMsg;
use crate::state::AppState;
use crate::state::conflict::ConflictView;
use crate::state::tool_compare::merging;
use std::time::Instant;
use trellis_core::repo::Choice;

pub(super) fn handle(st: &mut AppState, msg: ConflictMsg, fx: &mut Vec<Effect>) {
    match msg {
        ConflictMsg::OpenConflict(path) => {
            if st.workdir().is_none() {
                return;
            }
            super::repo::close_diff(st);
            super::history::close(st);
            super::blame::close(st);
            st.conflict = Some(ConflictView::loading(path));
            request_load(st, fx);
        }
        ConflictMsg::ConflictLoaded { path, generation, result } => {
            if generation != st.conflict_gen || st.conflict.as_ref().is_none_or(|c| c.path != path) {
                return; // superseded: closed, reopened or another file opened
            }
            match result {
                Ok(f) => {
                    if let Some(c) = st.conflict.as_mut() {
                        c.set_file(f);
                    }
                    st.view.conflict_scroll_to = Some(0);
                }
                Err(e) => {
                    st.log(false, e);
                    close(st);
                }
            }
        }
        ConflictMsg::CloseConflict => close(st),
        ConflictMsg::ConflictToggle { chunk, side } => {
            if let Some(c) = st.conflict.as_mut()
                && chunk < c.choices.len()
            {
                c.choices[chunk] = c.choices[chunk].toggle(side);
                c.current = chunk;
            }
        }
        ConflictMsg::ConflictTakeAll(side) => set_all(st, Choice::only(side)),
        ConflictMsg::ConflictReset => set_all(st, Choice::Unresolved),
        ConflictMsg::ConflictStep(d) => {
            let Some(c) = st.conflict.as_mut() else { return };
            let n = c.choices.len();
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
                && i < c.choices.len()
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
            super::ops::local(st, op, fx);
        }
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

fn set_all(st: &mut AppState, choice: Choice) {
    if let Some(c) = st.conflict.as_mut() {
        c.choices.iter_mut().for_each(|x| *x = choice);
    }
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

/// The working-tree status changed: once the open file is no longer conflicted (saved, marked
/// resolved, or resolved outside the app) the view closes. Picks are otherwise kept.
pub(super) fn status_changed(st: &mut AppState, now: Instant) {
    let (Some(c), Some(snap)) = (&st.conflict, &st.snap) else { return };
    let conflicted: Vec<&str> = snap.unstaged.iter().filter(|f| f.status == '!').map(|f| f.path.as_str()).collect();
    if conflicted.contains(&c.path.as_str()) {
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

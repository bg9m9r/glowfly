//! Picking in the conflict view: whole sides and single lines per chunk, Take all and Reset,
//! and editing the output as text (only the conflicts' text; a change anywhere else is undone).
//! A chunk edited by hand keeps its typed text through every pick until Discard edit.

use crate::state::AppState;
use crate::state::conflict::{ConflictView, OutputEdit};
use trellis_core::conflict_edit::{EditResult, edit_text, locate_edit};
use trellis_core::conflict_pick::{ChunkPick, file_eol};
use trellis_core::repo::{Choice, ConflictContent, Side};

/// What the editor says after an edit outside the conflicts was undone.
pub(crate) const OUTSIDE_NOTE: &str =
    "Only the conflicts can be edited here: that change touched other lines and was undone. Change the rest in your editor.";

/// Applies `f` to the pick of chunk `chunk` (unless its text was typed), making it current.
fn change(st: &mut AppState, chunk: usize, f: impl FnOnce(&ChunkPick, &trellis_core::repo::Chunk) -> ChunkPick) {
    let Some(v) = st.conflict.as_mut() else { return };
    let Some(pick) = v.picks.get(chunk).filter(|p| !p.is_edited()) else { return };
    let Some(c) = v.chunk(chunk) else { return };
    let next = f(pick, c);
    v.picks[chunk] = next;
    v.auto[chunk] = None;
    v.current = chunk;
    v.rebuild_edit();
}

pub(super) fn toggle_line(st: &mut AppState, chunk: usize, side: Side, line: usize) {
    change(st, chunk, |p, c| p.toggle_line(c, side, line));
}

pub(super) fn toggle_side(st: &mut AppState, chunk: usize, side: Side) {
    change(st, chunk, |p, c| p.toggle_side(c, side));
}

/// Take all and Reset: every chunk not edited by hand gets `choice`, and no pick is automatic
/// any more.
pub(super) fn set_all(st: &mut AppState, choice: Choice) {
    if let Some(c) = st.conflict.as_mut() {
        c.picks.iter_mut().filter(|p| !p.is_edited()).for_each(|p| *p = ChunkPick::Sides(choice));
        c.auto.iter_mut().for_each(|a| *a = None);
        c.rebuild_edit();
    }
}

/// Shows (`on`) or hides the output as editable text; only a text file has one.
pub(super) fn set_edit(st: &mut AppState, on: bool) {
    let Some(c) = st.conflict.as_mut() else { return };
    c.edit_note = None;
    c.edit = if on { start_edit(c) } else { None };
}

fn start_edit(c: &ConflictView) -> Option<OutputEdit> {
    let f = c.file.as_ref().filter(|f| matches!(f.content, ConflictContent::Text(_)))?;
    let rendered = edit_text(f, &c.picks)?;
    Some(OutputEdit { buffer: rendered.text.clone(), rendered })
}

/// The editor's text changed: a change inside one chunk becomes that chunk's typed text; any
/// other change is undone with a note.
pub(super) fn edited(st: &mut AppState) {
    let Some(c) = st.conflict.as_mut() else { return };
    let (Some(e), Some(ConflictContent::Text(segs))) = (&c.edit, c.file.as_ref().map(|f| &f.content)) else { return };
    match locate_edit(&e.rendered, &e.buffer, file_eol(segs)) {
        EditResult::Unchanged => {}
        EditResult::Chunk { chunk, lines } => {
            let before = match std::mem::take(&mut c.picks[chunk]) {
                ChunkPick::Edited { before, .. } => before,
                other => Box::new(other),
            };
            c.picks[chunk] = ChunkPick::Edited { lines, before };
            c.auto[chunk] = None;
            c.current = chunk;
            c.edit_note = None;
            c.rebuild_edit();
        }
        EditResult::Outside => {
            if let Some(e) = c.edit.as_mut() {
                e.buffer = e.rendered.text.clone();
            }
            c.edit_note = Some(OUTSIDE_NOTE.into());
        }
    }
}

/// Drops chunk `k`'s typed text, bringing back the pick it had before.
pub(super) fn discard_edit(st: &mut AppState, k: usize) {
    let Some(c) = st.conflict.as_mut() else { return };
    let Some(ChunkPick::Edited { before, .. }) = c.picks.get_mut(k) else { return };
    let before = std::mem::take(before.as_mut());
    c.picks[k] = before;
    c.current = k;
    c.rebuild_edit();
}

#[cfg(test)]
#[path = "conflict_edit_tests.rs"]
mod tests;

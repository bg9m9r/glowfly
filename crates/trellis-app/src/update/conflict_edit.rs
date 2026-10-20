//! Picking in the conflict view: whole sides and single lines per chunk, Take all and Reset,
//! and editing the output as text (one conflict or one clean run between conflicts at a time; a
//! change spanning both is undone). A chunk edited by hand keeps its typed text through every
//! pick until Discard edit; edited clean runs stay until Discard edits outside conflicts.

use crate::state::AppState;
use crate::state::conflict::{ConflictView, OutputEdit};
use trellis_core::conflict_edit::{EditResult, edit_text, locate_edit};
use trellis_core::conflict_pick::{ChunkPick, file_eol};
use trellis_core::repo::{Choice, ConflictContent, Segment, Side};

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
    let rendered = edit_text(f, &c.picks, &c.cleans)?;
    Some(OutputEdit { buffer: rendered.text.clone(), rendered })
}

/// `l` without its line terminator.
fn bare(l: &str) -> &str {
    l.strip_suffix('\n').map_or(l, |l| l.strip_suffix('\r').unwrap_or(l))
}

/// The editor's text changed: a change inside one chunk becomes that chunk's typed text, one
/// inside a clean run that run's (dropped once it reads as the file's own lines again); a
/// change spanning more than one is undone with a note.
pub(super) fn edited(st: &mut AppState) {
    let lang = st.settings.lang();
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
        EditResult::Clean { seg, lines } => {
            let Some(Segment::Clean(own)) = segs.get(seg) else { return };
            if lines.iter().map(|l| bare(l)).eq(own.iter().map(|l| bare(l))) {
                c.cleans.remove(&seg);
            } else {
                c.cleans.insert(seg, lines);
            }
            c.edit_note = None;
            c.rebuild_edit();
        }
        EditResult::Outside => {
            if let Some(e) = c.edit.as_mut() {
                e.buffer = e.rendered.text.clone();
            }
            // What the editor says after an edit spanning more than one region was undone.
            c.edit_note = Some(lang.tr("conflict.output.span_note").into());
        }
    }
}

/// Drops every edit of the clean runs, bringing back the file's own lines between the
/// conflicts.
pub(super) fn discard_cleans(st: &mut AppState) {
    let Some(c) = st.conflict.as_mut() else { return };
    c.cleans.clear();
    c.rebuild_edit();
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

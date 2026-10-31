//! Moving within the file history's File view: find in the shown version and go to line, as the
//! blame view does (`blame_find.rs`).

use crate::state::AppState;
use crate::state::blame_find::typed_line;
use crate::state::history::{FileHistoryView, FileTextData, HistoryPane, TextState};
use std::sync::Arc;

/// The text on screen, when it has loaded.
fn ready(view: &FileHistoryView) -> Option<Arc<FileTextData>> {
    let t = view.text.as_ref()?;
    let (oid, path, status) = view.shown()?;
    let shown = t.oid == oid && t.path == path && t.before == (status == 'D');
    match &t.state {
        TextState::Ready(d) if shown => Some(d.clone()),
        _ => None,
    }
}

/// Searches again for the Find field's text and goes to the first match at or after the top
/// line (else the first).
pub(super) fn find_changed(st: &mut AppState) {
    let Some(view) = st.session.history.view.as_mut() else { return };
    let lines = ready(view);
    let top = view.nav.top;
    let line = view.nav.find.refresh(lines.as_ref().map_or(&[][..], |d| &d.lines), top);
    if let Some(line) = line {
        view.nav.land_on(line);
    }
}

/// To the next (`d` > 0) or previous match, wrapping round.
pub(super) fn find_step(st: &mut AppState, d: isize) {
    let Some(view) = st.session.history.view.as_mut() else { return };
    if let Some(line) = view.nav.find.step(d) {
        view.nav.land_on(line);
    }
}

/// Focuses the Find field, which only the File pane shows (and not over a preview).
pub(super) fn focus_find(st: &mut AppState) {
    let preview_on = st.settings.history.preview;
    if let Some(view) = st.session.history.view.as_mut().filter(|v| v.pane == HistoryPane::File && !v.previewing(preview_on)) {
        view.nav.focus_find = true;
    }
}

/// Goes to the 1-based line typed in the Line field, clamped into the file; other text is
/// ignored.
pub(super) fn go_to_line(st: &mut AppState) {
    let Some(view) = st.session.history.view.as_mut() else { return };
    let Some(n) = ready(view).map(|d| d.lines.len()) else { return };
    if let Some(line) = typed_line(&view.nav.goto, n) {
        view.nav.land_on(line);
    }
}

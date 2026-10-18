//! The history's right panel: the selected file of a folder entry (and stepping through its
//! files), the Diff / File toggle (remembered in the settings), and the File view's text of
//! the selected version (`Effect::LoadFileText`).

use crate::effect::{Effect, FileTextRequest};
use crate::state::AppState;
use crate::state::blame_find::search;
use crate::state::history::{FileText, FileTextData, HistoryKind, HistoryPane, TextState};
use git2::Oid;
use std::sync::Arc;
use trellis_core::repo::DiffTarget;

/// Selects file `i` of the selected folder entry and opens that commit's diff of it.
pub(super) fn select_file(st: &mut AppState, i: usize, fx: &mut Vec<Effect>) {
    let Some(view) = st.history.as_mut().filter(|v| v.kind == HistoryKind::Folder) else { return };
    if view.selected_entry().is_none_or(|e| i >= e.files.len()) {
        return;
    }
    view.file = i;
    if let Some(target) = view.target() {
        super::repo::open_diff(st, target, fx);
    }
    want_text(st, fx);
}

/// Shows the selected commit's diff of the file, or the whole file as of it; later histories
/// open on the same pane.
pub(super) fn set_pane(st: &mut AppState, pane: HistoryPane, fx: &mut Vec<Effect>) {
    if st.history.is_none() {
        return;
    }
    if st.settings.history.pane != pane {
        st.settings.history.pane = pane;
        fx.push(super::prefs::save_settings(st));
    }
    if let Some(view) = st.history.as_mut() {
        view.pane = pane;
    }
    want_text(st, fx);
}

/// Asks for the shown file's text when the File view needs it.
pub(super) fn want_text(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    let smudge = st.runs_git_unasked();
    let Some(view) = st.history.as_mut() else { return };
    let Some((oid, path, before)) = view.wanted_text() else { return };
    view.text = Some(FileText { oid, path: path.clone(), before, state: TextState::Loading });
    view.nav.reset();
    fx.push(Effect::LoadFileText(FileTextRequest { workdir, oid, path, before, smudge }));
}

/// The text of `path` as of `oid` (or its parent) arrived: kept only while the view still waits
/// for it. The find field's matches are found again in it, without moving to one.
pub(super) fn text_loaded(st: &mut AppState, oid: Oid, path: &str, before: bool, result: Result<Arc<FileTextData>, String>) {
    let waiting = |t: &&mut FileText| t.oid == oid && t.path == path && t.before == before && matches!(t.state, TextState::Loading);
    let Some(view) = st.history.as_mut() else { return };
    let Some(text) = view.text.as_mut().filter(waiting) else { return };
    let failed = result.as_ref().err().cloned();
    if let Ok(data) = &result {
        view.nav.find.hits = search(&data.lines, &view.nav.find.query);
        view.nav.find.current = None;
    }
    text.state = match result {
        Ok(data) => TextState::Ready(data),
        Err(e) => TextState::Failed(e),
    };
    if let Some(e) = failed {
        st.log(false, e);
    }
}

/// Next/Previous change in a folder history's File pane: the neighbouring file of the selected
/// entry (`d` 1 or -1); nothing wraps.
pub(super) fn step_file(st: &mut AppState, d: isize, fx: &mut Vec<Effect>) {
    let Some(view) = st.history.as_ref().filter(|v| v.kind == HistoryKind::Folder && v.pane == HistoryPane::File) else { return };
    let Some(n) = view.selected_entry().map(|e| e.files.len()) else { return };
    let Some(j) = view.file.checked_add_signed(d).filter(|&j| j < n && d != 0) else { return };
    select_file(st, j, fx);
    st.view.history_file_scroll = Some(j);
}

/// A step of next/previous change opened `target`: in a folder history, when it is one of the
/// selected entry's files, that file becomes the selected one (and its text is asked for).
pub(super) fn follow_step(st: &mut AppState, target: &DiffTarget, fx: &mut Vec<Effect>) {
    let DiffTarget::Commit(oid, p) = target else { return };
    let Some(view) = st.history.as_mut().filter(|v| v.kind == HistoryKind::Folder) else { return };
    let Some(i) = view.selected_entry().filter(|e| e.oid == *oid).and_then(|e| e.files.iter().position(|f| f.path == *p)) else {
        return;
    };
    view.file = i;
    st.view.history_file_scroll = Some(i);
    want_text(st, fx);
}

/// After a tab switch: asks again for text whose result was dropped while the tab was parked.
pub(super) fn reload_text(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    let smudge = st.runs_git_unasked();
    let loading = st.history.as_ref().and_then(|v| v.text.as_ref()).filter(|t| matches!(t.state, TextState::Loading));
    if let Some(t) = loading {
        let (oid, path, before) = (t.oid, t.path.clone(), t.before);
        fx.push(Effect::LoadFileText(FileTextRequest { workdir, oid, path, before, smudge }));
    }
}

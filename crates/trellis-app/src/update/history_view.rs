//! The history's right panel: the selected file of a folder entry, the Diff / File toggle, and
//! the File view's text of the selected version (`Effect::LoadFileText`).

use crate::effect::{Effect, FileTextRequest};
use crate::state::AppState;
use crate::state::history::{FileText, FileTextData, HistoryKind, HistoryPane, TextState};
use git2::Oid;
use std::sync::Arc;

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

/// Shows the selected commit's diff of the file, or the whole file as of it.
pub(super) fn set_pane(st: &mut AppState, pane: HistoryPane, fx: &mut Vec<Effect>) {
    let Some(view) = st.history.as_mut() else { return };
    view.pane = pane;
    want_text(st, fx);
}

/// Asks for the shown file's text when the File view needs it.
pub(super) fn want_text(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    let Some(view) = st.history.as_mut() else { return };
    let Some((oid, path)) = view.wanted_text() else { return };
    view.text = Some(FileText { oid, path: path.clone(), state: TextState::Loading });
    fx.push(Effect::LoadFileText(FileTextRequest { workdir, oid, path }));
}

/// The text of `path` as of `oid` arrived: kept only while the view still waits for it.
pub(super) fn text_loaded(st: &mut AppState, oid: Oid, path: &str, result: Result<Arc<FileTextData>, String>) {
    let waiting = |t: &&mut FileText| t.oid == oid && t.path == path && matches!(t.state, TextState::Loading);
    let Some(text) = st.history.as_mut().and_then(|v| v.text.as_mut()).filter(waiting) else { return };
    let failed = result.as_ref().err().cloned();
    text.state = match result {
        Ok(data) => TextState::Ready(data),
        Err(e) => TextState::Failed(e),
    };
    if let Some(e) = failed {
        st.log(false, e);
    }
}

/// After a tab switch: asks again for text whose result was dropped while the tab was parked.
pub(super) fn reload_text(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    let loading = st.history.as_ref().and_then(|v| v.text.as_ref()).filter(|t| matches!(t.state, TextState::Loading));
    if let Some(t) = loading {
        fx.push(Effect::LoadFileText(FileTextRequest { workdir, oid: t.oid, path: t.path.clone() }));
    }
}

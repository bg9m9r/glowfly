//! The terminal panel header's own state: renaming a tab in place and maximising the panel
//! over the main area.

use crate::state::AppState;
use crate::state::terminal::{TabRename, TermSession};
use crate::state::terminal_names::{auto_name, clean_rename, tab_names};

/// Shows the rename field for tab `id` of the active repository, holding its name, and makes
/// it the active tab. The field takes the keyboard, so the shell doesn't ask for it (not even
/// for the click of the same double-click that selected the tab).
pub(super) fn start_rename(st: &mut AppState, id: u64) {
    let Some(workdir) = st.snap.as_ref().map(|s| s.workdir.clone()) else { return };
    let tabs: Vec<&TermSession> = st.terminal.sessions_for(&workdir).collect();
    let Some(i) = tabs.iter().position(|s| s.id == id) else { return };
    let Some(text) = tab_names(&tabs).into_iter().nth(i) else { return };
    st.terminal.active.insert(workdir, id);
    st.terminal.renaming = Some(TabRename { id, shown: text.clone(), text, focus: true });
    st.terminal.focus_requested = false;
}

/// Names tab `id` `name` from its rename field. A blank name, or the automatic one, goes back
/// to naming it automatically; the name the field opened with (which may carry the " (N)" of
/// a repeated name) leaves the tab named as it was. Enter (`submitted`) gives the keyboard back to the shell; when
/// the field lost focus to another widget, the keyboard stays with that widget.
pub(super) fn rename(st: &mut AppState, id: u64, name: &str, submitted: bool) {
    let Some(draft) = st.terminal.renaming.take_if(|r| r.id == id) else { return };
    st.terminal.focus_requested = submitted;
    let Some(s) = st.terminal.sessions.iter_mut().find(|s| s.id == id) else { return };
    let name = clean_rename(name);
    if name.as_deref() == clean_rename(&draft.shown).as_deref() {
        return;
    }
    s.name = name.filter(|n| *n != auto_name(s));
}

/// Esc in the rename field: the name stays and the shell gets the keyboard back.
pub(super) fn cancel_rename(st: &mut AppState) {
    st.terminal.renaming = None;
    st.terminal.focus_requested = true;
}

/// The panel takes the main area, or goes back under it. Only while it shows on a repository.
pub(super) fn toggle_maximize(st: &mut AppState) {
    if !st.terminal.open || st.snap.is_none() {
        return;
    }
    st.terminal.maximized = !st.terminal.maximized;
    st.terminal.focus_requested = true;
}

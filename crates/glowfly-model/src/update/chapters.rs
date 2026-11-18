//! The Chapters history view: the saved choice between the graph and the list, expanding chapters
//! and selecting them. The grouping itself is done by `settle`, after every message, when the
//! loaded history or the choice changed (state/chapters.rs).

use crate::effect::Effect;
use crate::msg::ChaptersMsg;
use crate::state::{self, AppState, Selection};
use glowfly_core::settings::HistoryMode;

pub(super) fn handle(st: &mut AppState, msg: ChaptersMsg, fx: &mut Vec<Effect>) {
    match msg {
        ChaptersMsg::SetMode(mode) => set_mode(st, mode, fx),
        ChaptersMsg::Toggle(id) => {
            st.session.chapters.toggle(id);
        }
        ChaptersMsg::Select(id) => {
            let Some(snap) = st.snap.as_ref() else { return };
            let Some(ch) = st.session.chapters.list.as_ref().and_then(|l| l.index_of(id).map(|i| &l.list[i])) else { return };
            let new = state::chapters::selection_of(ch, snap);
            super::selection::apply(st, new, fx);
        }
    }
}

fn set_mode(st: &mut AppState, mode: HistoryMode, fx: &mut Vec<Effect>) {
    let Some(real) = st.snap.as_ref().map(|s| s.real_workdir.clone()) else { return };
    let view = &mut st.session.chapters;
    if view.repo.as_deref() == Some(real.as_path()) && view.mode == mode {
        return;
    }
    view.repo = Some(real.clone());
    view.mode = mode;
    if st.settings.set_history_mode(&real, mode) {
        fx.extend(super::prefs::save_settings(st));
    }
    if mode == HistoryMode::Commits {
        let focus = match &st.session.selection {
            Selection::Commit(o) => Some(*o),
            Selection::VsWorkdir(w) => Some(w.oid),
            Selection::Commits(m) => Some(m.focus()),
            Selection::None | Selection::Wip => None,
        };
        if let Some(row) = focus.and_then(|oid| super::search::reveal(st, oid)) {
            st.view.scroll_to_row = Some(row.saturating_sub(4));
        }
    }
}

/// Keeps the Chapters view in step with the tab's snapshot: follows the repository's saved choice
/// and groups again when the history, HEAD or the refs changed. Cheap when nothing did. A tab whose
/// history was freed while parked keeps what it had until it is loaded again.
pub(super) fn settle(st: &mut AppState) {
    if st.session.tab_view.history_freed.is_some() {
        return;
    }
    let Some(snap) = st.snap.as_ref() else { return };
    st.session.chapters.follow(snap, &st.settings);
    st.session.chapters.sync(snap);
}

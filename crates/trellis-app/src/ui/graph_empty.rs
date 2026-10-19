//! The graph's empty states (`graph/mod.rs`): a repository with no commits, and a filtered
//! search with no matches, with Clear search, the query syntax and, when only part of the
//! history is loaded, Load more commits.

use super::empty_state::{EmptyIcon, PaneButton, empty_state};
use super::pane_status;
use crate::msg::{LoadMsg, Msg, SearchMsg};
use crate::state::AppState;
use crate::state::refresh::can_load_more;
use eframe::egui::Ui;

pub(super) const NO_COMMITS_TITLE: &str = "No commits yet";
const NO_COMMITS_HINT: &str = "Stage files and commit them to start this repository's history.";
const SEARCHING_FILES: &str = "Searching changed files…";

/// A repository without commits.
pub(super) fn no_commits(ui: &mut Ui) {
    empty_state(ui, EmptyIcon::Commits, NO_COMMITS_TITLE, NO_COMMITS_HINT, &[]);
}

/// The search filters the graph and nothing matches (or path terms are still being searched).
pub(super) fn no_matches(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    if st.search.searching_files() {
        return pane_status::loading(ui, SEARCHING_FILES);
    }
    let Some(snap) = &st.snap else { return };
    let t = st.settings.lang();
    let mut hint = t.tr("search.empty.hint").to_owned();
    if snap.truncated {
        hint.push(' ');
        hint.push_str(&t.trf("search.empty.truncated", &[("count", &snap.commits.len())]));
    }
    let more = can_load_more(snap);
    let mut list = vec![PaneButton { text: t.tr("search.clear.button"), primary: false }];
    if more {
        list.push(PaneButton { text: t.tr("search.load_more.button"), primary: false });
    }
    match empty_state(ui, EmptyIcon::Search, t.tr("search.empty.title"), &hint, &list) {
        Some(0) => msgs.push(Msg::Search(SearchMsg::Clear)),
        Some(_) => msgs.push(Msg::Load(LoadMsg::LoadMoreCommits)),
        None => {}
    }
}

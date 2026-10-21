//! The graph's empty states (`graph/mod.rs`): a repository with no commits, and a filtered
//! search with no matches. That pane says how far the search has looked: changed files or
//! older history still being searched, a failed load for more (Try again, Copy details), the
//! whole history searched, or only the newest commits, with Keep searching (or Load more
//! commits when history isn't loaded by itself), always with Clear search and the query syntax.

use super::empty_state::{EmptyIcon, PaneButton, empty_state};
use super::pane_status::{self, Failure};
use crate::msg::{LoadMsg, Msg, SearchMsg};
use crate::state::AppState;
use crate::state::load_more::{Further, Reach};
use eframe::egui::Ui;

pub(super) const NO_COMMITS_TITLE: &str = "No commits yet";
const NO_COMMITS_HINT: &str = "Stage files and commit them to start this repository's history.";
const SEARCHING_FILES: &str = "Searching changed files…";

/// A repository without commits.
pub(super) fn no_commits(ui: &mut Ui) {
    empty_state(ui, EmptyIcon::Commits, NO_COMMITS_TITLE, NO_COMMITS_HINT, &[]);
}

/// The search filters the graph and nothing matches (or it is still being searched).
pub(super) fn no_matches(ui: &mut Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    if st.search.searching_files() {
        return pane_status::loading(ui, SEARCHING_FILES);
    }
    let Some(snap) = &st.snap else { return };
    let t = st.settings.lang();
    let n = snap.commits.len();
    // The sentence after the syntax hint, and the button after Clear search with what it sends.
    let (reach, next) = match st.search_reach() {
        Reach::Searching => return pane_status::loading(ui, &t.trn("search.searching_older", n, &[])),
        Reach::Failed(error) => {
            let retry = Some(Msg::Load(LoadMsg::RetryLoadMore));
            return pane_status::failed(ui, t, Failure { title: t.tr("graph.load_more_failed.title"), error, retry }, msgs);
        }
        Reach::Whole { shallow: true } => (t.trn("search.empty.shallow", n, &[]), None),
        Reach::Whole { shallow: false } => (t.trn("search.empty.whole", n, &[]), None),
        Reach::Newest(Further::KeepSearching) => {
            (t.trn("search.empty.newest", n, &[]), Some((t.tr("search.keep_searching.button"), LoadMsg::SearchFurther)))
        }
        Reach::Newest(Further::LoadMore) => {
            (t.trf("search.empty.truncated", &[("count", &n)]), Some((t.tr("search.load_more.button"), LoadMsg::LoadMoreCommits)))
        }
        Reach::Newest(Further::Nothing) => (t.trf("search.empty.truncated", &[("count", &n)]), None),
    };
    let hint = format!("{} {reach}", t.tr("search.empty.hint"));
    let mut list = vec![PaneButton { text: t.tr("search.clear.button"), primary: false }];
    if let Some((text, _)) = &next {
        list.push(PaneButton { text, primary: false });
    }
    match empty_state(ui, EmptyIcon::Search, t.tr("search.empty.title"), &hint, &list) {
        Some(0) => msgs.push(Msg::Search(SearchMsg::Clear)),
        Some(_) => msgs.extend(next.map(|(_, m)| Msg::Load(m))),
        None => {}
    }
}

//! A load for more history that failed, its automatic retries, and a filtering search loading
//! further back. The fields live in `RefreshState` (`refresh.rs`); update/load_more.rs and
//! update/search_deeper.rs change them, and the graph reads them through `more_row` and
//! `search_reach`. Also the history's notes for the graph's end row (`HistoryNotes`, `end_row`):
//! how many commits a truncated history shows with Load more, and a shallow clone's Fetch full
//! history.

use crate::state::history_index::HistoryCount;
use crate::state::refresh::can_load_more;
use crate::state::shallow::history_remote;
use crate::state::{AppState, MAX_COMMITS};
use glowfly_core::commit_query::CommitQuery;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;
use std::time::Duration;

/// How long after a failed load for more each automatic retry waits; after the last one only
/// Try again loads again.
pub const AUTO_RETRY_DELAYS: [Duration; 2] = [Duration::from_secs(3), Duration::from_secs(10)];

/// How many batches of older history one filtering search loads by itself.
pub const DEEPER_BATCHES: u8 = 5;

/// The graph rows a filtering search wants to fill before the view has measured its viewport.
pub const FILL_ROWS_FALLBACK: usize = 20;

/// The filtering search loading older history, and how many batches it has asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct DeeperSearch {
    pub query: CommitQuery,
    pub batches: u8,
}

/// What the row under the last commit shows.
#[derive(Debug, PartialEq)]
pub enum MoreRow<'a> {
    /// More history is loading.
    Loading,
    /// A filtering search is loading older history for more matches.
    SearchingOlder,
    /// The last load for more failed with `error`; `retrying` while an automatic retry waits.
    Failed { error: &'a str, retrying: bool },
}

/// What the end row says when nothing is loading: the history stops short of the repository's
/// whole history, because of the commit limit (`truncated`) or a shallow clone.
#[derive(Debug, PartialEq)]
pub struct HistoryNotes {
    /// The commits loaded.
    pub commits: usize,
    /// Load more raises the commit limit.
    pub load_more: bool,
    /// Fetch full history can fetch the rest of a shallow clone.
    pub fetch_full: bool,
    pub shallow: bool,
    pub truncated: bool,
}

impl HistoryNotes {
    /// The notes for `snap`, if its history is cut short and it has commits.
    pub fn of(snap: &Snapshot) -> Option<Self> {
        let shallow = snap.is_shallow();
        if !(snap.truncated || shallow) || snap.commits.is_empty() {
            return None;
        }
        Some(Self {
            commits: snap.commits.len(),
            load_more: can_load_more(snap),
            fetch_full: shallow && history_remote(snap).is_some(),
            shallow,
            truncated: snap.truncated,
        })
    }

    /// The end row's sentence in `t`.
    pub fn sentence(&self, t: Lang) -> String {
        match (self.shallow, self.truncated) {
            (true, true) => t.trn("graph.end.shallow_newest", self.commits, &[]),
            (true, false) => t.trn("graph.end.shallow", self.commits, &[]),
            (false, _) => t.trn("graph.end.newest", self.commits, &[]),
        }
    }
}

/// The one row the graph shows after its last commit.
#[derive(Debug, PartialEq)]
pub enum EndRow<'a> {
    /// More history loads, a search looks further back, or the load for more failed.
    More(MoreRow<'a>),
    /// The history's end: how much shows, and what loads more.
    Notes(HistoryNotes),
}

/// How far a filtering search with no matches has looked.
#[derive(Debug, PartialEq)]
pub enum Reach<'a> {
    /// Older history is loading for it.
    Searching,
    /// Loading older history failed with this error.
    Failed(&'a str),
    /// Every commit was searched: the whole history of `commits` commits, or all a shallow clone
    /// has.
    Whole { shallow: bool, commits: usize },
    /// The whole-history index searched the newest `commits` commits (its cap) and found nothing.
    IndexCapped { commits: usize },
    /// The index found `matches` older commits that match, all past the newest `MAX_COMMITS` the
    /// graph can load.
    PastCap { matches: usize },
    /// Only the newest commits were searched, and what could look further.
    Newest(Further),
}

/// What can search further back than the loaded commits.
#[derive(Debug, PartialEq)]
pub enum Further {
    /// Another batch budget for the same query (the preference to load by itself is on).
    KeepSearching,
    /// Another commit limit's worth of history (the preference is off).
    LoadMore,
    /// Nothing: the history is loaded up to the cap.
    Nothing,
}

/// The wait before automatic retry number `retries + 1`, or None when they are used up.
pub fn retry_delay(retries: u8) -> Option<Duration> {
    AUTO_RETRY_DELAYS.get(usize::from(retries)).copied()
}

impl AppState {
    /// Whether the search bar filters the graph to a non-empty query's matches.
    pub fn filtering(&self) -> bool {
        self.view.show_search && self.session.search.filter && self.session.search.active()
    }

    /// Whether older history is loading for a filtering search.
    pub fn searching_older(&self) -> bool {
        self.filtering() && self.shows_loading_more_row()
    }

    /// The row the graph shows under its last commit, if any.
    pub fn more_row(&self) -> Option<MoreRow<'_>> {
        if self.shows_loading_more_row() {
            return Some(if self.filtering() { MoreRow::SearchingOlder } else { MoreRow::Loading });
        }
        let error = self.refresh.more_failed.as_deref().filter(|_| self.loading_more())?;
        Some(MoreRow::Failed { error, retrying: self.refresh.more_retry_at.is_some() })
    }

    /// The row the graph shows after its last commit, if any: the `more_row`, else the history's
    /// notes.
    pub fn end_row(&self) -> Option<EndRow<'_>> {
        if let Some(r) = self.more_row() {
            return Some(EndRow::More(r));
        }
        self.snap.as_ref().and_then(HistoryNotes::of).map(EndRow::Notes)
    }

    /// How far the filtering search has looked, for the pane shown when nothing matches.
    pub fn search_reach(&self) -> Reach<'_> {
        if self.searching_older() {
            return Reach::Searching;
        }
        if let Some(MoreRow::Failed { error, .. }) = self.more_row() {
            return Reach::Failed(error);
        }
        let Some(snap) = &self.snap else { return Reach::Newest(Further::Nothing) };
        if let HistoryCount::Whole { positions, commits, complete } = self.history_count() {
            match positions.first() {
                None if complete => return Reach::Whole { shallow: snap.is_shallow(), commits },
                None => return Reach::IndexCapped { commits },
                Some(&first) if self.session.search.matches.is_empty() && first as usize >= MAX_COMMITS => {
                    return Reach::PastCap { matches: positions.len() };
                }
                Some(_) => {}
            }
        }
        if !snap.truncated {
            Reach::Whole { shallow: snap.is_shallow(), commits: snap.commits.len() }
        } else if !can_load_more(snap) {
            Reach::Newest(Further::Nothing)
        } else if self.settings.load_more_on_scroll {
            Reach::Newest(Further::KeepSearching)
        } else {
            Reach::Newest(Further::LoadMore)
        }
    }

    /// The graph rows a filtering search tries to fill with matches.
    pub fn fill_rows(&self) -> usize {
        match self.view.graph_fit_rows {
            0 => FILL_ROWS_FALLBACK,
            n => n,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::settings::Settings;
    use std::time::Instant;

    #[test]
    fn retries_wait_longer_then_stop() {
        assert_eq!(retry_delay(0), Some(Duration::from_secs(3)));
        assert_eq!(retry_delay(1), Some(Duration::from_secs(10)));
        assert_eq!(retry_delay(2), None);
        assert_eq!(retry_delay(u8::MAX), None);
    }

    #[test]
    fn the_row_shows_loading_or_the_held_failure() {
        let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
        assert_eq!(st.more_row(), None, "nothing loaded");
        st.refresh.more_failed = Some("walk failed".into());
        assert_eq!(st.more_row(), None, "no snapshot, so no load for more");
        assert_eq!(st.fill_rows(), FILL_ROWS_FALLBACK);
        st.view.graph_fit_rows = 7;
        assert_eq!(st.fill_rows(), 7);

        let mut snap = crate::state::op_fixture::snapshot("more-row");
        let dir = snap.workdir.clone();
        snap.truncated = true;
        snap.loaded_with.max_commits = st.settings.commit_limit();
        st.snap = Some(snap);
        assert_eq!(st.more_row(), None, "no more was asked for: an ordinary refresh failed");
        st.refresh.extra_commits = st.settings.commit_limit();
        assert_eq!(st.more_row(), Some(MoreRow::Failed { error: "walk failed", retrying: false }));
        st.refresh.more_retry_at = Some(Instant::now());
        assert_eq!(st.more_row(), Some(MoreRow::Failed { error: "walk failed", retrying: true }));
        st.loading = true;
        assert_eq!(st.more_row(), Some(MoreRow::Loading), "a load runs");
        st.view.show_search = true;
        st.session.tab_view.search = "x".into();
        st.session.search.query = CommitQuery::parse("x");
        st.session.search.filter = true;
        assert_eq!(st.more_row(), Some(MoreRow::SearchingOlder));
        assert_eq!(st.search_reach(), Reach::Searching);
        st.loading = false;
        assert_eq!(st.search_reach(), Reach::Failed("walk failed"));
        st.refresh.more_failed = None;
        assert_eq!(st.search_reach(), Reach::Newest(Further::KeepSearching));
        st.settings.load_more_on_scroll = false;
        assert_eq!(st.search_reach(), Reach::Newest(Further::LoadMore));
        st.snap.as_mut().unwrap().truncated = false;
        assert_eq!(st.search_reach(), Reach::Whole { shallow: false, commits: 1 });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_index_answer_says_how_far_it_looked() {
        use crate::state::history_index::IndexAnswer;
        use glowfly_core::commit_index::{CommitIndex, IndexKey};
        use std::sync::Arc;
        let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
        let mut snap = crate::state::op_fixture::snapshot("reach-index");
        let dir = snap.workdir.clone();
        snap.truncated = true;
        let key = IndexKey::of(&snap);
        let commits = snap.commits.clone();
        st.snap = Some(snap);
        st.view.show_search = true;
        st.session.tab_view.search = "x".into();
        st.session.search.query = CommitQuery::parse("x");
        st.session.search.filter = true;
        let answer = |st: &mut AppState, ix: CommitIndex, positions: Vec<u32>| {
            let index = Arc::new(ix);
            st.session.search.index.ready = Some(Arc::clone(&index));
            st.session.search.index.answer = Some(IndexAnswer { query: st.session.search.query.clone(), index, positions });
        };
        answer(&mut st, CommitIndex::from_commits(key.clone(), &commits), vec![]);
        assert_eq!(st.search_reach(), Reach::Whole { shallow: false, commits: 1 });
        // A capped index found nothing in what it holds.
        answer(&mut st, CommitIndex::from_commits(key.clone(), &commits).marked_incomplete(), vec![]);
        assert_eq!(st.search_reach(), Reach::IndexCapped { commits: 1 });
        // Every match past the graph's cap.
        answer(&mut st, CommitIndex::from_commits(key, &commits), vec![250_000, 250_001]);
        assert_eq!(st.search_reach(), Reach::PastCap { matches: 2 });
        st.session.search.matches = vec![0];
        assert_eq!(st.search_reach(), Reach::Newest(Further::KeepSearching), "a loaded match: as before");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn history_notes_say_why_the_history_stops() {
        use glowfly_core::repo::RemoteInfo;
        let mut snap = crate::state::op_fixture::snapshot("history-notes");
        let dir = snap.workdir.clone();
        assert_eq!(HistoryNotes::of(&snap), None, "the whole history");
        snap.truncated = true;
        snap.loaded_with.max_commits = 2000;
        let n = HistoryNotes::of(&snap).unwrap();
        assert!(n.load_more && !n.fetch_full && n.truncated && !n.shallow);
        assert_eq!(n.sentence(Lang::En), "Showing the newest 1 commit.");
        snap.loaded_with.max_commits = MAX_COMMITS;
        assert!(!HistoryNotes::of(&snap).unwrap().load_more, "at the cap");
        snap.truncated = false;
        snap.shallow_roots = vec![snap.commits[0].oid];
        let n = HistoryNotes::of(&snap).unwrap();
        assert!(n.shallow && !n.fetch_full && !n.load_more, "shallow without a remote");
        assert_eq!(n.sentence(Lang::En), "Shallow clone: the history stops after 1 commit.");
        snap.remote_info = vec![RemoteInfo { name: "origin".into(), urls: vec!["u".into()], ..RemoteInfo::default() }];
        assert!(HistoryNotes::of(&snap).unwrap().fetch_full, "shallow with a remote");
        snap.truncated = true;
        snap.loaded_with.max_commits = 2000;
        let n = HistoryNotes::of(&snap).unwrap();
        assert!(n.fetch_full && n.load_more);
        assert_eq!(n.sentence(Lang::En), "Shallow clone: showing the newest 1 commit.");

        let mut st = AppState::new(Settings::default(), Vec::new(), Instant::now());
        st.snap = Some(snap);
        assert!(matches!(st.end_row(), Some(EndRow::Notes(_))));
        st.refresh.extra_commits = st.settings.commit_limit();
        st.loading = true;
        assert_eq!(st.end_row(), Some(EndRow::More(MoreRow::Loading)), "a pending load");
        st.loading = false;
        st.refresh.more_failed = Some("walk failed".into());
        assert!(matches!(st.end_row(), Some(EndRow::More(MoreRow::Failed { .. }))), "a failed load");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

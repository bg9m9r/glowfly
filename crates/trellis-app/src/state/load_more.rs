//! A load for more history that failed, its automatic retries, and a filtering search loading
//! further back. The fields live in `RefreshState` (`refresh.rs`); update/load_more.rs and
//! update/search_deeper.rs change them, and the graph reads them through `more_row` and
//! `search_reach`.

use crate::state::AppState;
use crate::state::refresh::can_load_more;
use std::time::Duration;
use trellis_core::commit_query::CommitQuery;

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

/// How far a filtering search with no matches has looked.
#[derive(Debug, PartialEq)]
pub enum Reach<'a> {
    /// Older history is loading for it.
    Searching,
    /// Loading older history failed with this error.
    Failed(&'a str),
    /// Every commit was searched: the whole history, or all a shallow clone has.
    Whole { shallow: bool },
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
        self.view.show_search && self.search.filter && self.search.active()
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

    /// How far the filtering search has looked, for the pane shown when nothing matches.
    pub fn search_reach(&self) -> Reach<'_> {
        if self.searching_older() {
            return Reach::Searching;
        }
        if let Some(MoreRow::Failed { error, .. }) = self.more_row() {
            return Reach::Failed(error);
        }
        let Some(snap) = &self.snap else { return Reach::Newest(Further::Nothing) };
        if !snap.truncated {
            Reach::Whole { shallow: snap.is_shallow() }
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
    use std::time::Instant;
    use trellis_core::settings::Settings;

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
        st.view.search = "x".into();
        st.search.query = CommitQuery::parse("x");
        st.search.filter = true;
        assert_eq!(st.more_row(), Some(MoreRow::SearchingOlder));
        assert_eq!(st.search_reach(), Reach::Searching);
        st.loading = false;
        assert_eq!(st.search_reach(), Reach::Failed("walk failed"));
        st.refresh.more_failed = None;
        assert_eq!(st.search_reach(), Reach::Newest(Further::KeepSearching));
        st.settings.load_more_on_scroll = false;
        assert_eq!(st.search_reach(), Reach::Newest(Further::LoadMore));
        st.snap.as_mut().unwrap().truncated = false;
        assert_eq!(st.search_reach(), Reach::Whole { shallow: false });
        let _ = std::fs::remove_dir_all(&dir);
    }
}

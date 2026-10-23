//! Using a tab's whole-history index for its search: asking it for the query's matches off the
//! UI thread (one request per edit, the newest replacing the rest), and loading exactly as far
//! back as the next match needs, for Enter, a filter that doesn't fill the graph, scrolling to
//! the end of a filtered list and Keep searching. Without a current answer every caller falls
//! back to the loaded-commits behaviour. Building the index is update/history_index.rs.

use crate::effect::{Effect, IndexEffect};
use crate::state::history_index::IndexAnswer;
use crate::state::{AppState, MAX_COMMITS};
use std::sync::Arc;

/// Asks the index for the query's matches unless it answered already or is answering: only for
/// an active query without path terms and an index current for the loaded history.
pub(super) fn want(st: &mut AppState, fx: &mut Vec<Effect>) {
    let current = st.search.active().then(|| st.current_index().cloned()).flatten();
    let Some(index) = current else {
        let ix = &mut st.search.index;
        ix.answer = None;
        ix.pending = None;
        ix.asked = None;
        return;
    };
    if st.index_answer().is_some() {
        return;
    }
    let query = st.search.query.clone();
    let ix = &mut st.search.index;
    let asking = ix.pending.is_some() && ix.asked.as_ref().is_some_and(|(q, i)| *q == query && Arc::ptr_eq(i, &index));
    if asking {
        return;
    }
    let Some(workdir) = st.snap.as_ref().map(|s| s.workdir.clone()) else { return };
    ix.requests += 1;
    ix.pending = Some(ix.requests);
    ix.asked = Some((query.clone(), Arc::clone(&index)));
    fx.push(Effect::Index(IndexEffect::Query { workdir, request: ix.requests, index, query }));
}

/// The index's answer to request `request`: kept when it is the one pending, then a filtering
/// search may load straight to its matches.
pub(super) fn queried(st: &mut AppState, request: u64, result: Result<Vec<u32>, String>, fx: &mut Vec<Effect>) {
    let ix = &mut st.search.index;
    if ix.pending != Some(request) {
        return;
    }
    ix.pending = None;
    let asked = ix.asked.take();
    match result {
        Ok(positions) => {
            if let Some((query, index)) = asked {
                ix.answer = Some(IndexAnswer { query, index, positions });
            }
        }
        Err(e) => st.log(false, format!("Could not search the history index: {e}")),
    }
    super::search_deeper::pump(st, fx);
}

/// The tab was shown again: answers in flight were dropped while it was parked (a loaded match is
/// asked for again), and its history may need a build.
pub(super) fn resume(st: &mut AppState, fx: &mut Vec<Effect>) {
    super::search_loaded::resume(st, fx);
    st.search.index.pending = None;
    st.search.index.asked = None;
    super::history_index::want_build(st, fx);
    want(st, fx);
}

/// Loads history through the `count` newest commits of the walk, rounded up to whole commit-limit
/// steps (`commits_wanted` caps it); false when that much is loaded or asked for already.
pub(super) fn load_through(st: &mut AppState, count: usize, fx: &mut Vec<Effect>) -> bool {
    let limit = st.settings.commit_limit().max(1);
    let extra = count.saturating_sub(limit).div_ceil(limit) * limit;
    if extra <= st.refresh.extra_commits {
        return false;
    }
    st.refresh.extra_commits = extra;
    st.refresh.more_retries = 0;
    super::refresh::refresh(st, fx);
    true
}

/// The loaded commit count and the answer's positions, when there is a current answer.
fn answer(st: &AppState) -> Option<(usize, &[u32])> {
    let a = st.index_answer()?;
    Some((st.snap.as_ref()?.commits.len(), &a.positions))
}

/// The commit count to load through for the first `n` matches: past the last of them the graph
/// can show. `None` when those are loaded already (or none can be).
fn through_matches(positions: &[u32], n: usize, loaded: usize) -> Option<usize> {
    let last = positions[..n.min(positions.len())].iter().rev().map(|&p| p as usize).find(|&p| p < MAX_COMMITS)?;
    (last >= loaded).then_some(last + 1)
}

/// Enter with no loaded match after the cursor: loads through the next older match and selects
/// it once loaded (`landed`). False when there is none the graph can show, so Enter wraps to the
/// newest match; a match past the graph's cap is only logged.
pub(super) fn next_beyond(st: &mut AppState, fx: &mut Vec<Effect>) -> bool {
    let Some((loaded, positions)) = answer(st) else { return false };
    if st.search.index.jump.is_some() && st.loading_more() {
        return true;
    }
    let Some(p) = positions.iter().map(|&p| p as usize).find(|&p| p >= loaded) else { return false };
    if p >= MAX_COMMITS {
        st.log(false, format!("The next match is older than the newest {MAX_COMMITS} commits the graph can show"));
        return false;
    }
    let oid = st.index_answer().and_then(|a| a.index.oid_at(p));
    st.search.index.jump = oid;
    load_through(st, p + 1, fx);
    true
}

/// A new history loaded: selects the older match Enter asked for once it is shown, or gives up
/// when the loads ended without it.
pub(super) fn landed(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(oid) = st.search.index.jump else { return };
    let Some(snap) = &st.snap else { return };
    if let Some(&row) = snap.row_of.get(&oid) {
        st.search.index.jump = None;
        st.search.cursor = row;
        st.search.stepped = true;
        super::repo::select_commit(st, oid, true, fx);
    } else if !st.loading_more() {
        st.search.index.jump = None;
        st.log_quiet(false, "The loaded history doesn't reach the next match");
    }
}

/// What a filtering search should load to fill the graph, from the index's answer: `None`
/// without one (the doubling batches decide), `Some(None)` when nothing needs loading (the
/// loaded matches fill the graph, no older match can be shown, or the answer is still coming),
/// else the commit count to load through for the first screenful of matches.
pub(super) fn fill_target(st: &AppState) -> Option<Option<usize>> {
    let Some((loaded, positions)) = answer(st) else {
        let asking = st.current_index().is_some() && st.search.index.pending.is_some();
        return asking.then_some(None);
    };
    let fill = st.fill_rows();
    if st.search.matches.len() >= fill {
        return Some(None);
    }
    Some(through_matches(positions, fill, loaded))
}

/// Scrolled to the end of a filtered list: loads through the next screenful of matches, or
/// nothing when no older match remains.
pub(super) fn near_end(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.loading_more() {
        return;
    }
    let Some((loaded, positions)) = answer(st) else { return };
    if let Some(count) = through_matches(positions, st.search.matches.len() + st.fill_rows(), loaded) {
        load_through(st, count, fx);
    }
}

/// Keep searching or Load more under no matches: the commit count to load through for the first
/// older match the graph can show.
pub(super) fn further_target(st: &AppState) -> Option<usize> {
    let (loaded, positions) = answer(st)?;
    positions.iter().map(|&p| p as usize).find(|&p| p >= loaded).filter(|&p| p < MAX_COMMITS).map(|p| p + 1)
}

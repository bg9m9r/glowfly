//! The search's path terms: asking the runner which loaded commits changed a matching path,
//! merging its reports, and keeping the answers across query changes, reloads and tab switches.
//! Answers are kept per set of path terms; a commit's changed paths never change, so they stay
//! valid by oid.

use crate::effect::Effect;
use crate::msg::PathsChunk;
use crate::state::AppState;
use crate::state::search::PathSearch;

/// Asks for the path answers the query still needs: resets them when the path terms changed,
/// then requests the loaded commits that pass the text and author terms and haven't been looked
/// at. A newer request makes the one in flight stale.
pub(super) fn want(st: &mut AppState, fx: &mut Vec<Effect>) {
    let s = &mut st.search;
    if !s.query.has_paths() {
        s.paths = PathSearch { requests: s.paths.requests, ..Default::default() };
        return;
    }
    if s.paths.terms != s.query.paths() {
        s.paths = PathSearch { terms: s.query.paths().to_vec(), requests: s.paths.requests, ..Default::default() };
        // The matches were found with the old terms' answers; none are known for these yet.
        super::search::recompute(st);
    }
    let s = &mut st.search;
    let Some(snap) = &st.snap else {
        s.paths.pending = None;
        return;
    };
    let oids: Vec<_> = snap.commits.iter().filter(|c| !s.paths.searched.contains(&c.oid) && s.query.matches(c)).map(|c| c.oid).collect();
    if oids.is_empty() {
        s.paths.pending = None;
        return;
    }
    s.paths.requests += 1;
    let request = s.paths.requests;
    s.paths.pending = Some(request);
    fx.push(Effect::SearchPaths { workdir: snap.workdir.clone(), request, terms: s.paths.terms.clone(), oids });
}

/// A report for request `request`: merges it when it is the one pending, and matches again.
/// Matches only grow below rows already shown, so nothing scrolls. When the search is done, a
/// filtering search may load further back.
pub(super) fn searched(st: &mut AppState, request: u64, result: Result<PathsChunk, String>, fx: &mut Vec<Effect>) {
    let p = &mut st.search.paths;
    if p.pending != Some(request) {
        return;
    }
    match result {
        Ok(chunk) => {
            p.searched.extend(chunk.searched);
            p.touching.extend(chunk.touching);
            if chunk.done {
                p.pending = None;
            }
        }
        Err(e) => {
            p.pending = None;
            st.log(false, format!("Could not search changed files: {e}"));
        }
    }
    super::search::recompute(st);
    if st.search.paths.pending.is_none() {
        super::search_deeper::pump(st, fx);
    }
}

/// The history changed: answers are kept for the commits still loaded, and any request in
/// flight is dropped (`want` asks again for what is unsearched).
pub(super) fn history_changed(st: &mut AppState) {
    let p = &mut st.search.paths;
    p.pending = None;
    match &st.snap {
        Some(snap) => {
            p.searched.retain(|o| snap.row_of.contains_key(o));
            p.touching.retain(|o| snap.row_of.contains_key(o));
        }
        None => {
            p.searched.clear();
            p.touching.clear();
        }
    }
}

/// The tab was shown again: its reports were dropped while it was parked, so ask afresh for
/// whatever is unsearched.
pub(super) fn resume(st: &mut AppState, fx: &mut Vec<Effect>) {
    st.search.paths.pending = None;
    want(st, fx);
}

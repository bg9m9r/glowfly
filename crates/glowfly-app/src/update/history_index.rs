//! Building a tab's whole-history search index (`state::history_index`): a build starts when a
//! large repository's history loads cut short and the index doesn't cover it yet (a fresh build
//! the first time, then updates from the previous index after a fetch, a commit or a branch
//! change), a finished build is kept even while the tab is parked, and closing the tab, opening
//! another repository in it or turning the preference off forgets it. Asking the index for a
//! query's matches is update/search_index.rs.
//!
//! With no index in memory, the build waits `FIRST_BUILD_DELAY` (driven by `Msg::Tick`, `tick`);
//! an update of an index in memory starts at once. A tab parked for `PARKED_FREE_AFTER` frees a
//! large index, and asks for it again (through the same wait) when shown. The runner reads the
//! saved index before a build without a base and saves the result (runner/index_disk.rs).

use crate::effect::{Effect, IndexEffect};
use crate::state::AppState;
use crate::state::history_index::{BuildDelay, Building, FIRST_BUILD_DELAY, FREE_MIN_COMMITS, HistoryIndex, PARKED_FREE_AFTER};
use glowfly_core::commit_index::{CommitIndex, IndexKey};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

/// Starts the build the loaded history needs, if any: none while the preference is off, the
/// graph holds the whole history, the index (or a build running) already covers it, or a build
/// for this same history failed. An index for another repository is forgotten first.
pub(super) fn want_build(st: &mut AppState, fx: &mut Vec<Effect>) {
    let workdir = st.snap.as_ref().map(|s| s.workdir.clone());
    let ix = &mut st.search.index;
    if ix.workdir != workdir {
        if let Some(old) = ix.workdir.take() {
            fx.push(Effect::Index(IndexEffect::Forget { workdir: old }));
        }
        ix.reset();
        ix.workdir = workdir;
    }
    let Some(snap) = &st.snap else { return };
    if !st.settings.index_history || !snap.truncated {
        return;
    }
    let ix = &mut st.search.index;
    let covered = |k: &IndexKey| k.covers(snap);
    if ix.ready.as_ref().is_some_and(|r| covered(r.key()))
        || ix.building.as_ref().is_some_and(|b| covered(&b.key))
        || ix.failed.as_ref().is_some_and(covered)
    {
        return;
    }
    // A first build (nothing in memory to update) waits for the tab to settle.
    if ix.ready.is_none() && ix.delay != BuildDelay::Passed {
        if ix.delay == BuildDelay::Idle {
            ix.delay = BuildDelay::Wanted;
        }
        return;
    }
    let key = IndexKey::of(snap);
    ix.builds += 1;
    // The core decides whether the previous index can be reused; it can't be when it was capped
    // or the shallow boundary moved, which is when the build starts from nothing.
    let base = ix.ready.clone();
    let fresh = base.as_ref().is_none_or(|b| !b.complete() || b.key().shallow != key.shallow);
    ix.building = Some(Building { build: ix.builds, key: key.clone(), indexed: 0, estimate: None, fresh });
    fx.push(Effect::Index(IndexEffect::Build { workdir: snap.workdir.clone(), build: ix.builds, key, base }));
}

/// A tick at `t`: the active tab's delayed build starts once due, and a parked tab's large index
/// is freed once the tab has been parked `PARKED_FREE_AFTER`. A build still running for a freed
/// index lands through `parked_built` and is freed again on a later tick.
pub(super) fn tick(st: &mut AppState, t: Instant, fx: &mut Vec<Effect>) {
    let ix = &mut st.search.index;
    ix.seen = Some(t);
    match ix.delay {
        BuildDelay::Wanted => ix.delay = BuildDelay::Due(t + FIRST_BUILD_DELAY),
        BuildDelay::Due(at) if t >= at => {
            ix.delay = BuildDelay::Passed;
            want_build(st, fx);
        }
        _ => {}
    }
    for tab in &mut st.tabs {
        let Some(p) = tab.parked.as_mut() else { continue };
        let ix = &mut p.commit_search.index;
        let since = *ix.seen.get_or_insert(t);
        let large = ix.ready.as_ref().is_some_and(|r| r.len() >= FREE_MIN_COMMITS);
        if large && t.saturating_duration_since(since) >= PARKED_FREE_AFTER {
            tracing::debug!(workdir = ?ix.workdir, "freeing a long-parked tab's search index");
            ix.free();
        }
    }
}

/// How far build `build` got (ignored for any other build).
pub(super) fn progress(st: &mut AppState, build: u64, indexed: usize, estimate: Option<usize>) {
    if let Some(b) = st.search.index.building.as_mut().filter(|b| b.build == build) {
        b.indexed = indexed;
        b.estimate = estimate;
    }
}

/// Build `build` finished: the index is kept, the history may already need another build (from
/// this one), and the query is asked of it.
pub(super) fn built(st: &mut AppState, build: u64, result: Result<Arc<CommitIndex>, String>, fx: &mut Vec<Effect>) {
    match finish(&mut st.search.index, build, result) {
        None => return,
        Some(Some(error)) => return st.log(false, failed_text(&error)),
        Some(None) => {}
    }
    want_build(st, fx);
    super::search_index::want(st, fx);
    super::search_deeper::pump(st, fx);
}

/// `built` for the parked tab of `workdir`: it keeps the index (or the failure) for when it is
/// shown again.
pub(super) fn parked_built(st: &mut AppState, workdir: &Path, build: u64, result: Result<Arc<CommitIndex>, String>) {
    let Some(i) = st.tabs.iter().position(|t| t.is(workdir) && t.parked.is_some()) else { return };
    let Some(p) = st.tabs[i].parked.as_mut() else { return };
    if let Some(Some(error)) = finish(&mut p.commit_search.index, build, result) {
        st.log_in(Some(workdir), false, failed_text(&error));
    }
}

fn failed_text(e: &str) -> String {
    format!("Could not index the history for search: {e}")
}

/// Applies build `build`'s result to `ix`: `None` when it isn't the build running, else the
/// error to log once. A failure is remembered for its history, so it isn't retried.
fn finish(ix: &mut HistoryIndex, build: u64, result: Result<Arc<CommitIndex>, String>) -> Option<Option<String>> {
    let b = ix.building.take_if(|b| b.build == build)?;
    Some(match result {
        Ok(index) => {
            ix.ready = Some(index);
            ix.failed = None;
            // Asked of the old index; a jump already loading still lands.
            ix.answer = None;
            ix.pending = None;
            ix.asked = None;
            None
        }
        Err(e) => {
            ix.failed = Some(b.key);
            Some(e)
        }
    })
}

/// Tab `i` is closing: its build stops and its index goes with the tab.
pub(super) fn tab_closed(st: &AppState, i: usize, fx: &mut Vec<Effect>) {
    let Some(tab) = st.tabs.get(i) else { return };
    let ix = if i == st.active_tab { Some(&st.search.index) } else { tab.parked.as_ref().map(|p| &p.commit_search.index) };
    if let Some(workdir) = ix.and_then(|x| x.workdir.clone()).or_else(|| tab.path.clone()) {
        fx.push(Effect::Index(IndexEffect::Forget { workdir }));
    }
}

/// The preference may have changed: off stops every tab's build and drops their indexes; on
/// starts the active tab's build (parked tabs build when shown).
pub(super) fn sync(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.settings.index_history {
        return want_build(st, fx);
    }
    let parked = st.tabs.iter_mut().filter_map(|t| t.parked.as_mut()).map(|p| &mut p.commit_search.index);
    for ix in std::iter::once(&mut st.search.index).chain(parked) {
        ix.delay = BuildDelay::Idle;
        if !ix.holds_any() && ix.answer.is_none() && ix.failed.is_none() {
            continue;
        }
        let workdir = ix.workdir.take();
        if let Some(w) = &workdir {
            fx.push(Effect::Index(IndexEffect::Forget { workdir: w.clone() }));
        }
        ix.reset();
        ix.workdir = workdir;
    }
}

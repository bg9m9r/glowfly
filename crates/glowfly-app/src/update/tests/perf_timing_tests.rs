//! Timings of the whole-history work `update` does on the UI thread (search, clean <-> dirty
//! relayout, branch hover, swapping snapshots), against a large repository named by
//! `GLOWFLY_PERF_REPO`. Ignored; run by hand in release:
//! `GLOWFLY_PERF_REPO=<repo> cargo test -p glowfly --release perf_timing -- --ignored --nocapture`.

use super::tests::state;
use crate::msg::{GraphMsg, Msg, SearchMsg};
use glowfly_core::repo::{LoadOptions, Snapshot, load};
use std::time::{Duration, Instant};

fn snapshot() -> Snapshot {
    let dir = std::env::var("GLOWFLY_PERF_REPO").expect("set GLOWFLY_PERF_REPO to a large repository");
    let t = Instant::now();
    let snap = load(std::path::Path::new(&dir), &LoadOptions::all(200_000)).unwrap();
    println!("load: {} commits in {:?}", snap.commits.len(), t.elapsed());
    snap
}

/// The median of `runs` timings of `f`.
fn median(runs: usize, mut f: impl FnMut()) -> Duration {
    let mut times: Vec<Duration> = (0..runs)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed()
        })
        .collect();
    times.sort();
    times[runs / 2]
}

#[test]
#[ignore]
fn perf_timing() {
    let mut st = state();
    st.snap = Some(snapshot());
    st.view.show_search = true;
    let now = Instant::now();

    // Typing a query: one message per keystroke, as the search field sends them.
    for q in ["p", "pa", "parser", "zzzz", "über", "author:ada", "author:ada parser"] {
        let key = median(5, || {
            st.session.tab_view.search = q.to_owned();
            let _ = super::update(&mut st, Msg::Search(SearchMsg::QueryChanged), now);
        });
        // What the pause's end costs the UI thread (asking the worker), what the worker spends,
        // and a match on the UI thread (Enter before the answer, a new history).
        let tick = median(5, || {
            st.session.tab_view.search = q.to_owned();
            super::search::query_changed(&mut st, false, &mut Vec::new());
        });
        let snap = st.snap.as_ref().unwrap();
        let query = glowfly_core::commit_query::CommitQuery::parse(q);
        let worker = median(5, || drop(query.matching_rows(&snap.commits, &Default::default())));
        let sync = median(5, || super::search::recompute(&mut st));
        println!(
            "search {q:?}: keystroke {key:?}, pause end {tick:?}, worker {worker:?}, UI-thread match {sync:?} ({} matches)",
            st.session.search.matches.len()
        );
    }
    st.session.tab_view.search.clear();
    super::search::recompute(&mut st);

    // The work tree going clean <-> dirty: before, a full relayout on the UI thread; now the UI
    // thread only takes the job (the refs' lane names) and swaps the worker's layout in.
    let d = median(5, || st.snap.as_mut().unwrap().lay_out_graph());
    println!("relayout on the UI thread: {d:?}");
    let job = median(5, || drop(st.snap.as_ref().unwrap().layout_job()));
    let worker = median(5, || drop(st.snap.as_ref().unwrap().layout_job().run()));
    let (fit, layout) = st.snap.as_ref().unwrap().layout_job().run();
    let t = Instant::now();
    let mut fx = Vec::new();
    super::large_graph::laid_out(&mut st, 0, &fit, layout, &mut fx);
    println!("relayout: UI thread {job:?} to ask + {:?} to swap in; worker {worker:?}", t.elapsed());

    // Hovering a branch chip, then another, then the first again: the first asks a worker for the
    // default branch's ancestry, which is then delivered.
    let tips: Vec<git2::Oid> = st.snap.as_ref().unwrap().locals.iter().filter_map(|b| b.oid).take(3).collect();
    let snap = st.snap.as_ref().unwrap();
    let tip = crate::state::ref_rows::default_tip(snap);
    let worker = median(3, || drop(crate::state::ref_rows::reachable_in(&snap.commits, tip)));
    let before = median(3, || drop(crate::state::ref_rows::default_ancestry(snap)));
    println!("ancestry: worker {worker:?} (UI thread before: {before:?})");
    for (n, &tip) in tips.iter().chain(tips.first()).enumerate() {
        let t = Instant::now();
        let fx = super::update(&mut st, Msg::Graph(GraphMsg::HoverRef(Some(tip))), now);
        println!("hover #{n}: {:?}", t.elapsed());
        for e in fx {
            if let Some(m) = crate::runner::testing::execute(e) {
                let t = Instant::now();
                let _ = super::update(&mut st, m, now);
                println!("  ancestry delivered: {:?}", t.elapsed());
            }
        }
        let _ = super::update(&mut st, Msg::Graph(GraphMsg::HoverRef(None)), now);
    }

    // A reload replacing the snapshot: before, the old one was freed here; now on a worker.
    let other = snapshot();
    let t = Instant::now();
    drop(st.snap.replace(other));
    println!("snapshot swap freeing on the UI thread: {:?}", t.elapsed());
    let other = snapshot();
    let t = Instant::now();
    let mut fx = Vec::new();
    super::large_graph::show(&mut st, other, &mut fx);
    println!("snapshot swap: {:?} ({} effects)", t.elapsed(), fx.len());
    let t = Instant::now();
    drop(fx);
    println!("  (the worker frees it in {:?})", t.elapsed());
}

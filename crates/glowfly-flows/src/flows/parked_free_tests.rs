//! A long-parked tab's history, freed and loaded again through the runner
//! (`update/parked_free.rs`): the tab comes back with the same history, selection and scroll.

use super::tests::{drive, finish, long_repo, scratch_repo, state};
use super::update;
use crate::msg::{Msg, RepoMsg, TabsMsg};
use crate::state::Selection;
use crate::state::tabs::history_free_wake;
use std::time::Instant;

#[test]
fn a_freed_history_comes_back_with_its_selection_and_scroll() {
    // Over the 5,000 commits a history must hold to be freed.
    let n = 6_000;
    let (a, b) = (long_repo("parked-free-flow-a", n + 50), scratch_repo("parked-free-flow-b"));
    let mut st = state();
    st.settings.initial_commits = n;
    drive(&mut st, Msg::Repo(RepoMsg::Open(a.clone())));
    let oid = st.snap.as_ref().unwrap().commits[5_000].oid;
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(oid)));
    st.session.tab_view.graph_scroll = (100_000.0, 600.0);
    drive(&mut st, Msg::Repo(RepoMsg::Open(b.clone())));
    assert_eq!((st.tabs.len(), st.active_tab), (2, 1));

    let t0 = Instant::now();
    let fx = update(&mut st, Msg::Tick(t0), Instant::now());
    finish(&mut st, fx);
    let due = history_free_wake(&st.tabs, t0).expect("A's history is to be freed");
    assert!(due > t0, "after a while parked");
    let fx = update(&mut st, Msg::Tick(due), Instant::now());
    finish(&mut st, fx);
    let p = st.tabs[0].parked.as_deref().unwrap();
    assert!(p.snap.as_ref().unwrap().commits.is_empty(), "A's history was freed");

    drive(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.commits.len(), n, "loaded as deep as before");
    assert_eq!(snap.commits[5_000].oid, oid);
    assert_eq!(st.session.tab_view.history_freed, None);
    assert_eq!(st.session.selection, Selection::Commit(oid));
    assert!(st.session.detail.as_ref().is_some_and(|d| d.oid == oid), "its details stay shown");
    assert_eq!(st.view.scroll_offset, Some(100_000.0));
    assert!(!st.loading);
    for d in [&a, &b] {
        let _ = std::fs::remove_dir_all(d);
    }
}

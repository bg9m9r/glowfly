//! Keeping the graph's top visible commit in place (`update/graph_anchor.rs`) through the real
//! runner: a Load more that turns into a full refresh because new commits moved the refs keeps
//! the commit at the top of the viewport where it was on screen.

use super::tests::{drive, finish, git, long_repo, state};
use super::update;
use glowfly_model::effect::{Effect, LoadEffect, LoadKind};
use glowfly_model::msg::{LoadMsg, Msg, RepoMsg};
use glowfly_model::state::Selection;
use std::time::Instant;

const ROW_H: f32 = 36.0;

#[test]
fn load_more_after_new_commits_keeps_the_top_commit() {
    let dir = long_repo("anchor-more", 1200);
    let mut st = state();
    st.settings.initial_commits = 500;
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    assert_eq!(st.snap.as_ref().map(|s| (s.truncated, s.commits.len())), Some((true, 500)), "cut short");
    // As drawn: the opening's scroll to the top taken, then scrolled down the graph.
    st.view.scroll_to_row = None;
    st.view.tab.graph_scroll = (100.0 * ROW_H + 5.0, 600.0);
    st.view.tab.graph_row_h = ROW_H;
    let (top, picked) = {
        let snap = st.snap.as_ref().unwrap();
        (snap.commits[100].oid, snap.commits[110].oid)
    };
    drive(&mut st, Msg::Repo(RepoMsg::SelectCommit(picked)));
    st.view.scroll_to_row = None;
    st.view.scroll_offset = None;

    for i in 0..3 {
        git(&dir, &["commit", "-q", "--allow-empty", "-m", &format!("on top {i}")]);
    }
    let fx = update(&mut st, Msg::Load(LoadMsg::LoadMoreCommits), Instant::now());
    let load = fx.into_iter().find(|e| matches!(e, Effect::Load(LoadEffect::Repo { kind: LoadKind::More, .. }))).expect("a load for more");
    let msg = glowfly_runner::testing::execute(load).unwrap();
    assert!(matches!(msg, Msg::Repo(RepoMsg::RepoLoaded { kind: LoadKind::Refresh, .. })), "the refs moved: a full refresh");
    let fx = update(&mut st, msg, Instant::now());
    finish(&mut st, fx);

    let snap = st.snap.as_ref().unwrap();
    assert_eq!(snap.commits.len(), 1000);
    assert_eq!(snap.row_of.get(&top), Some(&103));
    assert_eq!(st.view.scroll_offset, Some(103.0 * ROW_H + 5.0));
    assert_eq!(st.view.scroll_to_row, None);
    assert_eq!(st.session.selection, Selection::Commit(picked));
    let _ = std::fs::remove_dir_all(&dir);
}

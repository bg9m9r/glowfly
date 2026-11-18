//! Search under a `.mailmap` on `long_repo`'s 620 commits, 500 loaded: an author term matches
//! the mapped address in the loaded commits and in the whole-history index, and an edit of the
//! `.mailmap` is shown by the next refresh without the index being built again.

use super::history_index_tests::{past_the_delay, run_build};
use super::tests::{finish, finish_keeping, long_repo, state};
use super::update;
use glowfly_model::effect::{Effect, IndexEffect};
use glowfly_model::msg::{Msg, RepoMsg, SearchMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::history_index::HistoryCount;
use std::sync::Arc;
use std::time::Instant;

fn search(st: &mut AppState, q: &str) -> Vec<Effect> {
    st.view.show_search = true;
    st.view.tab.search = q.into();
    update(st, Msg::Search(SearchMsg::QueryChanged), Instant::now())
}

fn whole(st: &AppState) -> usize {
    match st.history_count() {
        HistoryCount::Whole { positions, .. } => positions.len(),
        other => panic!("the index answered: {other:?}"),
    }
}

#[test]
fn a_mailmap_edit_asks_the_index_again_without_a_rebuild() {
    let dir = long_repo("search-mailmap-edit", 620);
    std::fs::write(dir.join(".mailmap"), "Proper Name <proper@example.com> <test@example.com>\n").unwrap();
    let mut st = state();
    st.settings.initial_commits = 500;
    let fx = update(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())), Instant::now());
    let mut kept = finish_keeping(&mut st, fx);
    kept.extend(past_the_delay(&mut st));
    let fx = run_build(&mut st, kept);
    finish(&mut st, fx);
    let built = st.session.search.index.ready.clone().expect("the index is built");
    assert_eq!(built.len(), 620);

    let fx = search(&mut st, "author:proper@example.com");
    finish(&mut st, fx);
    assert_eq!(st.session.search.matches.len(), 500, "every loaded commit is by Proper Name");
    assert_eq!(st.history_count(), HistoryCount::Whole { positions: &(0..620).collect::<Vec<u32>>(), commits: 620, complete: true });
    let key = st.snap.as_ref().unwrap().refs_key;

    std::fs::remove_file(dir.join(".mailmap")).unwrap();
    let fx = update(&mut st, Msg::Repo(RepoMsg::Refresh), Instant::now());
    let kept = finish_keeping(&mut st, fx);
    assert!(kept.iter().all(|e| !matches!(e, Effect::Index(IndexEffect::Build { .. }))), "no index build: {kept:?}");
    let snap = st.snap.as_ref().unwrap();
    assert_ne!(snap.refs_key, key, "the load was a full one");
    assert!(snap.mailmap.is_empty() && snap.commits.iter().all(|c| c.author.name() == "Test"));
    assert!(Arc::ptr_eq(&built, st.session.search.index.ready.as_ref().unwrap()), "the index is the one built before");
    assert!(st.session.search.matches.is_empty(), "no loaded match under the old address");
    assert_eq!(whole(&st), 0, "asked again: the old address matches nothing");

    let fx = search(&mut st, "author:test@example.com");
    finish(&mut st, fx);
    assert_eq!(st.session.search.matches.len(), 500);
    assert_eq!(whole(&st), 620);
    let _ = std::fs::remove_dir_all(&dir);
}

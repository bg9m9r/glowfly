//! The Chapters view through the runner: loading more history goes on under the chapters (the
//! oldest chapter grows and keeps its id), and the saved choice comes back when a repository is
//! opened again.

use super::tests::{drive, finish, long_repo, state};
use super::update;
use glowfly_core::settings::HistoryMode;
use glowfly_model::effect::Effect;
use glowfly_model::msg::{ChaptersMsg, LoadMsg, Msg, RepoMsg};
use glowfly_model::state::AppState;
use std::time::Instant;

fn opened(name: &str, count: usize, limit: usize) -> AppState {
    let dir = long_repo(name, count);
    let mut st = state();
    st.settings.initial_commits = limit;
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir)));
    st
}

#[test]
fn load_more_goes_on_under_the_chapters() {
    let mut st = opened("chapters-more", 700, 500);
    drive(&mut st, Msg::Chapters(ChaptersMsg::SetMode(HistoryMode::Chapters)));
    let list = st.session.chapters.list.as_ref().unwrap();
    assert_eq!((list.list.len(), list.list[0].count(), list.open_end), (1, 500, true));
    let id = list.list[0].id;
    let fx: Vec<Effect> = update(&mut st, Msg::Load(LoadMsg::LoadMoreCommits), Instant::now());
    finish(&mut st, fx);
    let list = st.session.chapters.list.as_ref().unwrap();
    assert_eq!((list.list.len(), list.list[0].id, list.list[0].count(), list.open_end), (1, id, 700, false));
}

#[test]
fn the_choice_survives_reopening() {
    let dir = long_repo("chapters-reopen", 30);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(dir.clone())));
    drive(&mut st, Msg::Chapters(ChaptersMsg::SetMode(HistoryMode::Chapters)));
    let mut again = state();
    again.settings = st.settings.clone();
    drive(&mut again, Msg::Repo(RepoMsg::Open(dir)));
    assert_eq!(again.session.chapters.mode, HistoryMode::Chapters);
    assert!(again.session.chapters.list.is_some());
}

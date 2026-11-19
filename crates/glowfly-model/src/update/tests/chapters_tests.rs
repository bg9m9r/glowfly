//! Headless tests of the Chapters history view (`update/chapters.rs`): the per-repository
//! choice and its saving, grouping after each message, expanding, selecting a chapter (a lone
//! commit, a merge or a run, also of two), loading more history and going back to the graph.

use super::tests::{git, history_repo, long_repo, state};
use super::update;
use crate::effect::{CommitEffect, Effect, LoadKind, SaveEffect, SelectionEffect};
use crate::msg::{ChaptersMsg, Msg, RepoMsg, SelectionMsg};
use crate::state::chapters::{Row, is_selected};
use crate::state::graph_rows::GraphRows;
use crate::state::selection_span::Span;
use crate::state::{AppState, Selection, Tab};
use git2::Oid;
use glowfly_core::repo::{self, Comparison, LoadOptions, Snapshot};
use glowfly_core::settings::HistoryMode;
use std::path::Path;
use std::time::Instant;

fn snap_of(dir: &Path, n: usize) -> Snapshot {
    repo::load(dir, &LoadOptions::all(n)).unwrap()
}

fn showing(dir: &Path, n: usize) -> AppState {
    let mut st = state();
    st.tabs[0] = Tab::unloaded(dir.to_path_buf());
    st.snap = Some(snap_of(dir, n));
    st
}

fn send(st: &mut AppState, m: ChaptersMsg) -> Vec<Effect> {
    update(st, Msg::Chapters(m), Instant::now())
}

fn in_chapters(dir: &Path) -> AppState {
    let mut st = showing(dir, 500);
    send(&mut st, ChaptersMsg::SetMode(HistoryMode::Chapters));
    st
}

fn real(st: &AppState) -> std::path::PathBuf {
    st.snap.as_ref().unwrap().real_workdir.clone()
}

fn oid_of(st: &AppState, summary: &str) -> Oid {
    st.snap.as_ref().unwrap().commits.iter().find(|c| &*c.summary == summary).unwrap().oid
}

/// "first" by bob, then "two" and "three" by ada (one run of two).
fn two_over_one(name: &str) -> std::path::PathBuf {
    history_repo(name, &[("bob", 1000, "first"), ("ada", 2000, "two"), ("ada", 2100, "three")])
}

#[test]
fn the_switch_defaults_to_commits() {
    let dir = two_over_one("chapters-default");
    let mut st = showing(&dir, 500);
    send(&mut st, ChaptersMsg::Toggle(Oid::from_bytes(&[3; 20]).unwrap()));
    assert_eq!(st.session.chapters.mode, HistoryMode::Commits);
    assert!(st.session.chapters.list.is_none());
}

#[test]
fn set_mode_saves_once_per_change() {
    let dir = two_over_one("chapters-save");
    let mut st = showing(&dir, 500);
    let fx = send(&mut st, ChaptersMsg::SetMode(HistoryMode::Chapters));
    let [Effect::Save(SaveEffect::Settings(saved))] = fx.as_slice() else { panic!("one save, got {fx:?}") };
    assert_eq!(saved.history_mode_for(&real(&st)), HistoryMode::Chapters);
    assert!(st.session.chapters.list.is_some(), "grouped at once");
    assert!(send(&mut st, ChaptersMsg::SetMode(HistoryMode::Chapters)).is_empty());
    let fx = send(&mut st, ChaptersMsg::SetMode(HistoryMode::Commits));
    let [Effect::Save(SaveEffect::Settings(saved))] = fx.as_slice() else { panic!("one save, got {fx:?}") };
    assert!(saved.repo_history_modes.is_empty());
    assert!(st.session.chapters.list.is_none());
}

#[test]
fn the_choice_is_remembered_per_repository() {
    let (a, b) = (two_over_one("chapters-remember-a"), two_over_one("chapters-remember-b"));
    let mut first = showing(&a, 500);
    send(&mut first, ChaptersMsg::SetMode(HistoryMode::Chapters));
    let settings = first.settings.clone();
    let mut other = showing(&b, 500);
    other.settings = settings.clone();
    send(&mut other, ChaptersMsg::Toggle(Oid::from_bytes(&[3; 20]).unwrap()));
    assert_eq!(other.session.chapters.mode, HistoryMode::Commits);
    let mut again = showing(&a, 500);
    again.settings = settings;
    send(&mut again, ChaptersMsg::Toggle(Oid::from_bytes(&[3; 20]).unwrap()));
    assert_eq!(again.session.chapters.mode, HistoryMode::Chapters);
    assert_eq!(again.session.chapters.list.as_ref().unwrap().list.len(), 2);
}

#[test]
fn toggling_expands_and_collapses() {
    let dir = history_repo("chapters-toggle", &[("ada", 1000, "a"), ("ada", 1100, "b"), ("ada", 1200, "c"), ("ada", 1300, "d")]);
    let mut st = in_chapters(&dir);
    let id = st.session.chapters.list.as_ref().unwrap().list[0].id;
    send(&mut st, ChaptersMsg::Toggle(id));
    assert!(st.session.chapters.expanded.contains(&id));
    assert_eq!(st.session.chapters.rows.len(), 5);
    assert_eq!(st.session.chapters.rows[0], Row::Chapter(0));
    send(&mut st, ChaptersMsg::Toggle(id));
    assert_eq!(st.session.chapters.rows, [Row::Chapter(0)]);
    send(&mut st, ChaptersMsg::Toggle(Oid::from_bytes(&[3; 20]).unwrap()));
    assert_eq!(st.session.chapters.rows, [Row::Chapter(0)]);
}

#[test]
fn selecting_a_run_selects_its_line() {
    let dir = history_repo("chapters-run", &[("bob", 1000, "a"), ("ada", 1100, "b"), ("ada", 1200, "c"), ("ada", 1300, "d")]);
    let mut st = in_chapters(&dir);
    let id = st.session.chapters.list.as_ref().unwrap().list[0].id;
    let fx = send(&mut st, ChaptersMsg::Select(id));
    let Selection::Commits(m) = &st.session.selection else { panic!("a multi-selection: {:?}", st.session.selection) };
    assert_eq!(m.oids, [oid_of(&st, "d"), oid_of(&st, "c"), oid_of(&st, "b")]);
    assert_eq!(m.span, Span::Range { base: oid_of(&st, "a"), newest: oid_of(&st, "d") });
    assert!(fx.iter().any(
        |e| matches!(e, Effect::Selection(SelectionEffect::Compare { from, to, .. }) if *from == oid_of(&st, "a") && *to == oid_of(&st, "d"))
    ));
    let snap = st.snap.as_ref().unwrap();
    assert!(is_selected(&st.session.chapters.list.as_ref().unwrap().list[0], snap, &st.session.selection));
}

#[test]
fn a_run_reaching_the_root_combines_from_the_empty_tree() {
    let dir = history_repo("chapters-root", &[("ada", 1000, "a"), ("ada", 1100, "b"), ("ada", 1200, "c")]);
    let mut st = in_chapters(&dir);
    let id = st.session.chapters.list.as_ref().unwrap().list[0].id;
    send(&mut st, ChaptersMsg::Select(id));
    let Selection::Commits(m) = &st.session.selection else { panic!("a multi-selection") };
    assert_eq!(m.span, Span::Range { base: repo::empty_tree(), newest: oid_of(&st, "c") });
}

#[test]
fn a_run_of_two_combines_from_the_older_ones_parent() {
    let dir = two_over_one("chapters-two");
    let mut st = in_chapters(&dir);
    let id = st.session.chapters.list.as_ref().unwrap().list[0].id;
    let fx = send(&mut st, ChaptersMsg::Select(id));
    let (first, three) = (oid_of(&st, "first"), oid_of(&st, "three"));
    let Selection::Commits(m) = &st.session.selection else { panic!("a multi-selection") };
    assert_eq!(m.span, Span::Range { base: first, newest: three }, "not a pair");
    assert!(fx.iter().any(|e| matches!(e, Effect::Selection(SelectionEffect::Compare { from, to, .. }) if *from == first && *to == three)));
    update(
        &mut st,
        Msg::Selection(SelectionMsg::CompareLoaded { from: first, to: three, result: Ok(Comparison::default()) }),
        Instant::now(),
    );
    // The same history again: the selection stays a line and keeps its file list.
    let result = Ok(Box::new(snap_of(&dir, 500)));
    let generation = st.load_gen;
    update(&mut st, Msg::Repo(RepoMsg::RepoLoaded { generation, kind: LoadKind::Refresh, result }), Instant::now());
    let Selection::Commits(m) = &st.session.selection else { panic!("still a multi-selection") };
    assert_eq!(m.span, Span::Range { base: first, newest: three });
    assert!(m.changes.is_some(), "the received file list is kept");
}

#[test]
fn an_ordinary_pair_is_still_a_pair() {
    let dir = two_over_one("chapters-pair");
    let mut st = showing(&dir, 500);
    let (two, three) = (oid_of(&st, "two"), oid_of(&st, "three"));
    update(&mut st, Msg::Repo(RepoMsg::SelectCommit(three)), Instant::now());
    update(&mut st, Msg::Selection(SelectionMsg::ToggleCommit(two)), Instant::now());
    let Selection::Commits(m) = &st.session.selection else { panic!("a pair") };
    assert!(matches!(m.span, Span::Pair { .. }));
}

#[test]
fn selecting_a_merge_or_single_chapter_selects_its_commit() {
    let dir = history_repo("chapters-merge", &[("ada", 1000, "a"), ("bob", 1100, "b")]);
    git(&dir, &["checkout", "-q", "-b", "side", "HEAD~1"]);
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "s1"]);
    git(&dir, &["checkout", "-q", "main"]);
    git(&dir, &["merge", "-q", "--no-ff", "side", "-m", "Merge branch 'side'"]);
    let mut st = in_chapters(&dir);
    let id = st.session.chapters.list.as_ref().unwrap().list[0].id;
    let fx = send(&mut st, ChaptersMsg::Select(id));
    assert_eq!(st.session.selection, Selection::Commit(id));
    assert!(fx.iter().any(|e| matches!(e, Effect::Commit(CommitEffect::LoadDetail { oid, .. }) if *oid == id)));
    // An inside commit selected on its own does not light the chapter row.
    let side = oid_of(&st, "s1");
    update(&mut st, Msg::Repo(RepoMsg::SelectCommit(side)), Instant::now());
    let snap = st.snap.as_ref().unwrap();
    assert!(!is_selected(&st.session.chapters.list.as_ref().unwrap().list[0], snap, &st.session.selection));
}

#[test]
fn a_longer_history_extends_the_list() {
    let dir = long_repo("chapters-longer", 60);
    let mut st = showing(&dir, 20);
    send(&mut st, ChaptersMsg::SetMode(HistoryMode::Chapters));
    let list = st.session.chapters.list.clone().unwrap();
    assert_eq!((list.list.len(), list.list[0].count(), list.open_end), (1, 20, true));
    let id = list.list[0].id;
    send(&mut st, ChaptersMsg::Toggle(id));
    let result = Ok(Box::new(snap_of(&dir, 100)));
    let generation = st.load_gen;
    update(&mut st, Msg::Repo(RepoMsg::RepoLoaded { generation, kind: LoadKind::Refresh, result }), Instant::now());
    let list = st.session.chapters.list.as_ref().unwrap();
    assert_eq!((list.list.len(), list.list[0].id, list.list[0].count(), list.open_end), (1, id, 60, false));
    assert!(st.session.chapters.expanded.contains(&id));
    assert_eq!(st.session.chapters.rows.len(), 61);
}

#[test]
fn switching_back_to_commits_reveals_the_selection() {
    let dir = long_repo("chapters-reveal", 30);
    let mut st = in_chapters(&dir);
    let oid = st.snap.as_ref().unwrap().commits[15].oid;
    update(&mut st, Msg::Repo(RepoMsg::SelectCommit(oid)), Instant::now());
    st.view.scroll_to_row = None;
    send(&mut st, ChaptersMsg::SetMode(HistoryMode::Commits));
    let snap = st.snap.as_ref().unwrap();
    let row = GraphRows::new(snap, &st.session.search, st.view.show_search).row_of(oid).unwrap();
    assert_eq!(st.view.scroll_to_row, Some(row - 4));
}

#[test]
fn a_freed_history_keeps_the_list() {
    let dir = two_over_one("chapters-freed");
    let mut st = in_chapters(&dir);
    let id = st.session.chapters.list.as_ref().unwrap().list[0].id;
    send(&mut st, ChaptersMsg::Toggle(id));
    st.session.tab_view.history_freed = Some(500);
    st.snap = Some(snap_of(&dir, 1));
    send(&mut st, ChaptersMsg::Toggle(Oid::from_bytes(&[3; 20]).unwrap()));
    let view = &st.session.chapters;
    assert!(view.list.is_some() && view.expanded.contains(&id));
    assert_eq!(view.list.as_ref().unwrap().list.len(), 2);
}

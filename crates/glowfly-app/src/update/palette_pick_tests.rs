//! Headless tests for the palette's file finder: when "File history…" is offered, entering and
//! leaving the sub-list, loading the tracked files (and ignoring stale results), ranking and
//! capping the rows, and opening the picked file's history.

use super::palette_tests::repo_with_topic;
use super::tests::{drive, git, state};
use super::update;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::msg::PaletteMsg;
use crate::msg::RepoMsg;
use crate::state::conflict::ConflictView;
use crate::state::history::FileHistoryView;
use crate::state::palette::{Items, MAX_PICK_ROWS, Mode, Palette, PaletteCommand, Pick, PickKind, row_count, shown};
use crate::state::rebase::RebasePanel;
use crate::state::{AppState, Modal};
use glowfly_core::rebase::RebaseBase;
use std::time::Instant;

const FILE_HISTORY: &str = "File history…";

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn listed(st: &AppState) -> bool {
    shown(st, "").iter().any(|s| s.entry.label == FILE_HISTORY)
}

fn pick(st: &AppState) -> &Pick {
    match st.palette.as_ref().map(|p| &p.mode) {
        Some(Mode::Pick(p)) => p,
        _ => panic!("the palette is not in a sub-list"),
    }
}

fn pick_mut(st: &mut AppState) -> &mut Pick {
    match st.palette.as_mut().map(|p| &mut p.mode) {
        Some(Mode::Pick(p)) => p,
        _ => panic!("the palette is not in a sub-list"),
    }
}

fn loaded(st: &mut AppState, generation: u64, result: Result<Vec<String>, String>) {
    run(st, Msg::Palette(PaletteMsg::TrackedPathsLoaded { generation, result }));
}

fn enter(st: &mut AppState) -> Vec<Effect> {
    run(st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::FileHistory)))
}

/// A palette in the file finder over `items`, outside any repository.
fn picking(items: Vec<String>) -> AppState {
    let mut st = state();
    let mut pk = Pick::new(PickKind::FileHistory);
    pk.set_items(Items::Ready(items));
    st.palette = Some(Palette { mode: Mode::Pick(pk), ..Default::default() });
    st
}

fn set_query(st: &mut AppState, q: &str) {
    st.palette.as_mut().unwrap().query = q.into();
}

/// The labels of the shown rows, after ranking for the current query.
fn rows(st: &mut AppState) -> Vec<String> {
    row_count(st);
    let pk = pick(st);
    pk.ranked().unwrap().hits.iter().map(|h| pk.label(h.index).to_owned()).collect()
}

#[test]
fn file_history_entry_availability() {
    let st = state();
    assert!(!listed(&st), "no repository");

    let (mut st, dir) = repo_with_topic("pick-entry");
    assert!(listed(&st));
    let labels: Vec<String> = shown(&st, "").into_iter().map(|s| s.entry.label).collect();
    let at = labels.iter().position(|l| l == FILE_HISTORY).unwrap();
    assert_eq!(labels[at - 1], "Search commits", "right after Search commits");

    st.history = Some(FileHistoryView::loading("a.txt".into(), None));
    assert!(!listed(&st));
    st.history = None;
    st.rebase = Some(RebasePanel::loading(RebaseBase::OntoBranch("topic".into())));
    assert!(!listed(&st));
    st.rebase = None;
    st.conflict = Some(ConflictView::loading("a.txt".into()));
    assert!(!listed(&st));
    st.conflict = None;
    assert!(listed(&st));
    let _ = std::fs::remove_dir_all(&dir);

    // No commits yet: HEAD is unborn.
    let empty = glowfly_core::paths::temp_dir().join(format!("glowfly-app-test-pick-unborn-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&empty);
    std::fs::create_dir_all(&empty).unwrap();
    git(&empty, &["init", "-q", "-b", "main"]);
    let mut st = state();
    drive(&mut st, Msg::Repo(RepoMsg::Open(empty.clone())));
    assert!(st.snap.is_some());
    assert!(!listed(&st));
    let _ = std::fs::remove_dir_all(&empty);
}

#[test]
fn file_history_enters_pick_mode() {
    let (mut st, dir) = repo_with_topic("pick-enter");
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    set_query(&mut st, "fil");
    let fx = enter(&mut st);
    let p = st.palette.as_ref().expect("still open");
    assert!(p.query.is_empty());
    assert_eq!(p.selected, 0);
    assert_eq!(pick(&st).kind, PickKind::FileHistory);
    assert_eq!(pick(&st).items, Items::Loading);
    let gen_now = st.palette_gen;
    assert!(matches!(&fx[..], [Effect::LoadTrackedPaths { workdir, generation }] if *workdir == dir && *generation == gen_now));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_history_list_loads() {
    let (mut st, dir) = repo_with_topic("pick-load");
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    drive(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::FileHistory)));
    assert!(matches!(&pick(&st).items, Items::Ready(v) if v.iter().any(|p| p == "a.txt")));
    assert!(row_count(&mut st) > 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stale_tracked_paths_ignored() {
    let (mut st, dir) = repo_with_topic("pick-stale");
    let ok = || Ok(vec!["a.txt".to_owned()]);
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    enter(&mut st);
    let g = st.palette_gen;
    loaded(&mut st, g - 1, ok());
    assert_eq!(pick(&st).items, Items::Loading);

    // Back in the command list: the result has nowhere to go.
    run(&mut st, Msg::Palette(PaletteMsg::PaletteBack));
    loaded(&mut st, g, ok());
    assert!(matches!(st.palette.as_ref().unwrap().mode, Mode::Commands));

    // Closed: nothing changes.
    run(&mut st, Msg::Palette(PaletteMsg::ClosePalette));
    loaded(&mut st, g, ok());
    assert!(st.palette.is_none());

    // Entered again: only the new request's result counts.
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    enter(&mut st);
    assert_eq!(st.palette_gen, g + 1);
    loaded(&mut st, g, ok());
    assert_eq!(pick(&st).items, Items::Loading);
    loaded(&mut st, g + 1, ok());
    assert_eq!(pick(&st).items, Items::Ready(vec!["a.txt".into()]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tracked_paths_failure_shows_and_logs() {
    let (mut st, dir) = repo_with_topic("pick-fail");
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    enter(&mut st);
    let before = st.log.len();
    let g = st.palette_gen;
    loaded(&mut st, g, Err("boom".into()));
    assert_eq!(pick(&st).items, Items::Failed("boom".into()));
    assert_eq!(st.log.len(), before + 1);
    assert!(!st.log.last().unwrap().ok);
    run(&mut st, Msg::Palette(PaletteMsg::PaletteMove(1)));
    assert_eq!(st.palette.as_ref().unwrap().selected, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn pick_ranking_caps_and_narrows() {
    let items: Vec<String> = (0..500).map(|i| format!("src/mod{i}/file{i}.rs")).collect();
    let mut st = picking(items.clone());
    assert_eq!(row_count(&mut st), MAX_PICK_ROWS);
    let r = pick(&st).ranked().unwrap();
    assert_eq!(r.total, 500);
    assert!(r.hits.iter().enumerate().all(|(i, h)| h.index == i && h.matched.is_empty()), "item order");

    set_query(&mut st, "file42");
    assert_eq!(rows(&mut st)[0], "src/mod42/file42.rs");

    // A longer query narrows the last matches; the result is the same as ranking afresh.
    set_query(&mut st, "file42.r");
    let narrowed = rows(&mut st);
    let total = pick(&st).ranked().unwrap().total;
    let mut fresh = picking(items.clone());
    set_query(&mut fresh, "file42.r");
    assert_eq!(narrowed, rows(&mut fresh));
    assert_eq!(total, pick(&fresh).ranked().unwrap().total);
    assert!(total < 500 && total > 0);

    // A shorter query ranks all items again.
    set_query(&mut st, "file4");
    let shorter = rows(&mut st);
    let mut fresh = picking(items);
    set_query(&mut fresh, "file4");
    assert_eq!(shorter, rows(&mut fresh));
    assert!(pick(&st).ranked().unwrap().total > total);

    // The same query isn't ranked twice; new items drop the ranking.
    pick_mut(&mut st).set_items(Items::Ready(vec!["file4.rs".into()]));
    assert!(pick(&st).ranked().is_none());
    assert_eq!(rows(&mut st), ["file4.rs"]);

    // Moving wraps within the shown rows.
    let mut st = picking((0..500).map(|i| format!("f{i}")).collect());
    run(&mut st, Msg::Palette(PaletteMsg::PaletteMove(-1)));
    assert_eq!(st.palette.as_ref().unwrap().selected, MAX_PICK_ROWS - 1);
}

#[test]
fn picking_a_file_opens_history() {
    let (mut st, dir) = repo_with_topic("pick-open");
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    enter(&mut st);
    drive(&mut st, Msg::Palette(PaletteMsg::RunPaletteCommand(PaletteCommand::OpenFileHistory("a.txt".into()))));
    assert!(st.palette.is_none());
    let h = st.history.as_ref().expect("history open");
    assert_eq!(h.path, "a.txt");
    assert_eq!(h.at, None);
    assert!(!h.loading);
    assert!(!h.entries.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn back_returns_to_commands() {
    let mut st = picking(vec!["a.txt".into()]);
    set_query(&mut st, "x");
    st.palette.as_mut().unwrap().selected = 3;
    run(&mut st, Msg::Palette(PaletteMsg::PaletteBack));
    let p = st.palette.as_ref().unwrap();
    assert!(matches!(p.mode, Mode::Commands));
    assert!(p.query.is_empty());
    assert_eq!(p.selected, 0);

    // In the command list: nothing changes, the palette stays open.
    set_query(&mut st, "pu");
    run(&mut st, Msg::Palette(PaletteMsg::PaletteBack));
    assert_eq!(st.palette.as_ref().map(|p| p.query.as_str()), Some("pu"));

    // Ctrl+P in the sub-list closes the palette.
    let mut st = picking(vec!["a.txt".into()]);
    run(&mut st, Msg::Palette(PaletteMsg::TogglePalette));
    assert!(st.palette.is_none());
    // Closed: going back does nothing.
    run(&mut st, Msg::Palette(PaletteMsg::PaletteBack));
    assert!(st.palette.is_none());
}

#[test]
fn file_history_when_closed_opens_palette() {
    let (mut st, dir) = repo_with_topic("pick-closed");
    let fx = enter(&mut st);
    assert_eq!(pick(&st).kind, PickKind::FileHistory);
    assert!(matches!(&fx[..], [Effect::LoadTrackedPaths { .. }]));

    st.palette = None;
    st.modal =
        Some(Modal::Confirm { title: "t".into(), body: "b".into(), button: "OK".into(), then: Box::new(Msg::Repo(RepoMsg::Refresh)) });
    assert!(enter(&mut st).is_empty());
    assert!(st.palette.is_none());

    // Nor without a repository.
    let mut st = state();
    assert!(enter(&mut st).is_empty());
    assert!(st.palette.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

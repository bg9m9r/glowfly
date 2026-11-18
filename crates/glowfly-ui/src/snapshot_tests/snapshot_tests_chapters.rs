//! The Chapters history view: the switch above the graph, the list (the work tree's row, release
//! headings, chapters with their counts, authors and dates), expanded chapters, clicks and keys,
//! the search bar bringing the graph back, and the empty state. Shares the fixture repository and
//! renderer with `snapshot_tests`.

use super::snapshot_tests::{git, harness, reload, remove_fixture, render, with_repo};
use super::snapshot_tests_esc::app_harness;
use crate::snapshot_check::CheckSnapshot;
use crate::snapshot_check::text_snapshot;
use eframe::egui::{Event, Key, Modifiers};
use egui_kittest::kittest::Queryable;
use git2::Oid;
use glowfly_core::repo::LoadOptions;
use glowfly_core::settings::{HistoryMode, ThemeChoice};
use glowfly_model::msg::{ChaptersMsg, LoadMsg, Msg, RepoMsg};
use glowfly_model::state::chapters::selection_of;
use glowfly_model::state::{AppState, Selection};
use std::path::PathBuf;

/// The fixture with three commits on 2026-01-12 (09:00, 10:00 and 11:30, the last closing issue
/// 7: one chapter titled by it) and "Add dark theme (#9)" on the 14th, tagged `v0.2.0`, reloaded
/// in full and shown as chapters.
fn in_chapters(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, name);
    let at = |day: u32, h: u32, m: u32| format!("2026-01-{day:02}T{h:02}:{m:02}:00+00:00");
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "Add search box"], &at(12, 9, 0));
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "Highlight matches"], &at(12, 10, 0));
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "Search results list", "-m", "Closes #7"], &at(12, 11, 30));
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "Add dark theme (#9)"], &at(14, 10, 0));
    git(&dir, &["tag", "v0.2.0"], &at(14, 10, 0));
    reload(&mut st, &dir, &LoadOptions::all(500));
    show_chapters(&mut st);
    (st, dir)
}

fn show_chapters(st: &mut AppState) {
    st.session.chapters.mode = HistoryMode::Chapters;
    st.session.chapters.sync(st.snap.as_ref().unwrap());
}

fn chapter_id(st: &AppState, title: &str) -> Oid {
    st.session.chapters.list.as_ref().unwrap().list.iter().find(|c| c.title == title).unwrap().id
}

fn commit(st: &AppState, summary: &str) -> Oid {
    st.snap.as_ref().unwrap().commits.iter().find(|c| c.summary == summary).unwrap().oid
}

#[test]
fn chapters_collapsed() {
    let (st, dir) = in_chapters("chapters-collapsed");
    let (run, merge) = (chapter_id(&st, "Search results list"), chapter_id(&st, "login-page"));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("chapters_collapsed");
    assert!(h.query_by_label("Add dark theme (#9), 1 commit").is_some());
    assert!(h.query_by_label("Release tag v0.2.0").is_some() && h.query_by_label("Release tag v0.1.0").is_some());
    h.get_by_label("login-page, 2 commits").click();
    h.run();
    assert!(matches!(&h.state().1[..], [Msg::Chapters(ChaptersMsg::Select(o))] if *o == merge));
    h.state_mut().1.clear();
    h.get_by_label("Show the commits of Search results list").click();
    h.run();
    assert!(matches!(&h.state().1[..], [Msg::Chapters(ChaptersMsg::Toggle(o))] if *o == run));
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn chapters_expanded() {
    let (mut st, dir) = in_chapters("chapters-expanded");
    let (run, merge) = (chapter_id(&st, "Search results list"), chapter_id(&st, "login-page"));
    st.session.chapters.toggle(run);
    st.session.chapters.toggle(merge);
    let inside = commit(&st, "Highlight matches");
    st.session.selection = Selection::Commit(inside);
    let mut h = harness(st, true);
    h.run_ok();
    h.check_snapshot("chapters_expanded");
    assert!(h.query_by_label("Hide the commits of login-page").is_some());
    assert!(h.query_by_label("Hide the commits of Search results list").is_some());
    h.state_mut().1.clear();
    h.get_by_label("Highlight matches").click();
    h.run_ok();
    assert!(matches!(&h.state().1[..], [Msg::Repo(RepoMsg::SelectCommit(o))] if *o == inside), "{}", h.state().1.len());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn chapters_release() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "chapters-release");
    let at = |day: u32, h: u32| format!("2026-01-{day:02}T{h:02}:00:00+00:00");
    for (i, msg) in ["Draft the page", "Style the page", "Link the page", "Ship the page"].into_iter().enumerate() {
        git(&dir, &["commit", "-q", "--allow-empty", "-m", msg], &at(12, 9 + i as u32));
        if msg == "Style the page" {
            git(&dir, &["tag", "v0.3.0"], &at(12, 10));
            git(&dir, &["tag", "nightly"], &at(12, 10));
        }
    }
    reload(&mut st, &dir, &LoadOptions::all(500));
    show_chapters(&mut st);
    let list = st.session.chapters.list.as_ref().unwrap();
    assert_eq!(list.list.iter().take(2).map(|c| c.title.as_str()).collect::<Vec<_>>(), ["Link the page", "Draft the page"]);
    render("chapters_release", st, true);
    remove_fixture(&dir);
}

#[test]
fn the_switch_sends_the_mode() {
    let (st, dir) = with_repo(ThemeChoice::Dark, "chapters-switch");
    let mut h = harness(st, true);
    h.run();
    h.get_by_label("Chapters").click();
    h.run();
    assert!(matches!(&h.state().1[..], [Msg::Chapters(ChaptersMsg::SetMode(HistoryMode::Chapters))]));
    drop(h);
    let (st, dir2) = in_chapters("chapters-switch-back");
    let mut h = harness(st, true);
    h.run();
    h.state_mut().1.clear();
    h.get_by_label("Commits").click();
    h.run();
    assert!(matches!(&h.state().1[..], [Msg::Chapters(ChaptersMsg::SetMode(HistoryMode::Commits))]));
    drop(h);
    remove_fixture(&dir);
    remove_fixture(&dir2);
}

#[test]
fn an_open_search_shows_the_commits_view() {
    let (mut st, dir) = in_chapters("chapters-search");
    st.view.show_search = true;
    let mut h = harness(st, true);
    h.run();
    let text = text_snapshot(&h);
    assert!(text.contains("Commit graph") || h.query_by_label("Commit graph").is_some(), "the graph is drawn");
    assert!(h.query_by_label("Add dark theme (#9), 1 commit").is_none(), "not the chapters");
    let chapters_line = text.lines().find(|l| l.contains("\"Chapters\"")).unwrap_or_default();
    assert!(chapters_line.contains("disabled"), "the Chapters segment is disabled: {chapters_line}");
    drop(h);
    remove_fixture(&dir);
}

/// Presses `key` in the whole app and returns the messages it caused.
fn press(h: &mut egui_kittest::Harness<'_, (AppState, Vec<Msg>)>, key: Key) -> Vec<Msg> {
    h.state_mut().1.clear();
    h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
    h.step();
    std::mem::take(&mut h.state_mut().1)
}

#[test]
fn arrow_keys_move_over_chapters() {
    let (mut st, dir) = in_chapters("chapters-keys");
    let (run, merge) = (chapter_id(&st, "Search results list"), chapter_id(&st, "login-page"));
    let snap = st.snap.as_ref().unwrap();
    let ch = st.session.chapters.list.as_ref().unwrap().list.iter().find(|c| c.id == run).unwrap();
    st.session.selection = selection_of(ch, snap);
    let mut h = app_harness(st);
    h.run_steps(4);
    // Down skips the release heading above the merge chapter.
    let msgs = press(&mut h, Key::ArrowDown);
    assert!(matches!(&msgs[..], [Msg::Chapters(ChaptersMsg::Select(o))] if *o == merge), "{}", msgs.len());
    // Right expands the current chapter.
    let msgs = press(&mut h, Key::ArrowRight);
    assert!(matches!(&msgs[..], [Msg::Chapters(ChaptersMsg::Toggle(o))] if *o == run), "{}", msgs.len());
    // Left on a commit goes to its chapter.
    {
        let st = &mut h.state_mut().0;
        st.session.chapters.toggle(run);
        st.session.selection = Selection::Commit(commit(st, "Highlight matches"));
    }
    h.run_steps(2);
    let msgs = press(&mut h, Key::ArrowLeft);
    assert!(matches!(&msgs[..], [Msg::Chapters(ChaptersMsg::Select(o))] if *o == run), "{}", msgs.len());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn an_unborn_head_shows_the_empty_state() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "chapters-unborn");
    let snap = st.snap.as_mut().unwrap();
    snap.head_oid = None;
    snap.graph.wip = None;
    show_chapters(&mut st);
    let mut h = harness(st, true);
    h.run();
    let text = text_snapshot(&h);
    assert!(text.contains("No chapters") && text.contains("The current branch has no commits yet."), "{text}");
    drop(h);
    remove_fixture(&dir);
}

fn asks_for_more(msgs: &[Msg]) -> bool {
    msgs.iter().any(|m| matches!(m, Msg::Load(LoadMsg::NearHistoryEnd)))
}

/// The chapters of a history cut at the commit limit: `truncated`, with the first-parent line going on.
fn truncated_chapters(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = in_chapters(name);
    let limit = st.settings.commit_limit();
    let snap = st.snap.as_mut().unwrap();
    snap.truncated = true;
    snap.loaded_with.max_commits = limit;
    st.session.chapters.list.as_mut().unwrap().open_end = true;
    (st, dir)
}

#[test]
fn the_list_asks_for_more_history_near_its_end_once_per_row_count() {
    let (st, dir) = truncated_chapters("chapters-near-end");
    let mut h = harness(st, true);
    h.run();
    assert!(asks_for_more(&h.state().1), "every row is visible, so the end is near");
    // The load finished without adding a row: the list doesn't ask again by itself, the end row's
    // Load more is what is left.
    h.state_mut().1.clear();
    h.run_steps(4);
    assert!(!asks_for_more(&h.state().1), "a load that adds no rows isn't followed by another");
    h.get_by_label("Load more").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Load(LoadMsg::LoadMoreCommits))));

    // A load that added rows asks again.
    h.state_mut().1.clear();
    let at = "2026-01-15T10:00:00+00:00";
    git(&dir, &["commit", "-q", "--allow-empty", "-m", "Rename the page"], at);
    let st = &mut h.state_mut().0;
    reload(st, &dir, &LoadOptions::all(500));
    let limit = st.settings.commit_limit();
    let snap = st.snap.as_mut().unwrap();
    snap.truncated = true;
    snap.loaded_with.max_commits = limit;
    show_chapters(st);
    st.session.chapters.list.as_mut().unwrap().open_end = true;
    h.run();
    assert!(asks_for_more(&h.state().1), "the row count changed");
    drop(h);

    let (mut st, _) = truncated_chapters("chapters-near-end");
    st.settings.load_more_on_scroll = false;
    let mut h = harness(st, true);
    h.run();
    assert!(!asks_for_more(&h.state().1), "the preference is off");
    drop(h);
    let (mut st, _) = truncated_chapters("chapters-near-end");
    st.session.chapters.list.as_mut().unwrap().open_end = false;
    let mut h = harness(st, true);
    h.run();
    assert!(!asks_for_more(&h.state().1), "the whole line is shown");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn an_empty_list_with_more_history_below_loads_it_instead_of_saying_no_chapters() {
    let (mut st, dir) = truncated_chapters("chapters-open-empty");
    let snap = st.snap.as_mut().unwrap();
    snap.graph.wip = None;
    let list = st.session.chapters.list.as_mut().unwrap();
    list.list.clear();
    st.session.chapters.rows.clear();
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("No chapters").is_none(), "the empty state would hide the end row");
    assert!(asks_for_more(&h.state().1), "the end is near");
    h.get_by_label("Load more").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Load(LoadMsg::LoadMoreCommits))));
    drop(h);
    remove_fixture(&dir);
}

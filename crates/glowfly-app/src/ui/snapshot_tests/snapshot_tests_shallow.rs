//! A shallow clone in the graph's end row (its sentence and Fetch full history, also beside Load
//! more, cut in a narrow window, and without a remote) and the palette's commit count step for
//! fetching more.

use super::snapshot_tests::{harness, harness_sized, remove_fixture, render, with_repo};
use crate::msg::{JobMsg, Msg};
use crate::state::AppState;
use crate::state::palette::{InputKind, Mode, Palette};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use glowfly_core::repo::RemoteInfo;
use glowfly_core::settings::ThemeChoice;
use glowfly_core::shallow_ops::Deepen;

/// Marks the fixture shallow at its oldest commit, with an origin remote.
fn make_shallow(st: &mut AppState) {
    let snap = st.snap.as_mut().unwrap();
    snap.shallow_roots = vec![snap.commits.last().unwrap().oid];
    snap.remote_info.retain(|r| r.name != "origin");
    snap.remote_info.push(RemoteInfo {
        name: "origin".into(),
        urls: vec!["https://example.com/app.git".into()],
        push_urls: vec![],
        skip_fetch_all: false,
    });
}

#[test]
fn graph_end_row_shallow() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "shallow");
    make_shallow(&mut st);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("graph_shallow");
    let n = h.state().0.snap.as_ref().unwrap().commits.len();
    assert_sentence_and_button(&h, &format!("Shallow clone: the history stops after {n} commits."), "Fetch full history");
    assert!(h.query_by_label("Load more").is_none(), "the whole of what the clone has is loaded");
    h.get_by_label("Fetch full history").hover();
    h.run();
    h.check_snapshot("graph_shallow_tooltip");
    h.get_by_label("Fetch full history").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Job(JobMsg::Deepen(Deepen::Full)))));
    remove_fixture(&dir);
}

#[test]
fn graph_end_row_shallow_and_truncated() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "shallow-truncated");
    make_shallow(&mut st);
    st.snap.as_mut().unwrap().truncated = true;
    st.settings.columns.commit = false;
    st.settings.columns.date = false;
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("graph_shallow_load_more_light");
    let n = h.state().0.snap.as_ref().unwrap().commits.len();
    assert_sentence_and_button(&h, &format!("Shallow clone: showing the newest {n} commits."), "Fetch full history");
    let fetch = h.get_by_label("Fetch full history").bounding_box().unwrap();
    let more = h.get_by_label("Load more").bounding_box().unwrap();
    assert!(more.x0 > fetch.x1, "Load more is rightmost");
    remove_fixture(&dir);
}

/// The end row (named by its sentence) after the last commit's row, with `button` in it.
fn assert_sentence_and_button(h: &egui_kittest::Harness<'_, (AppState, Vec<crate::msg::Msg>)>, sentence: &str, button: &str) {
    let last = h.state().0.snap.as_ref().unwrap().commits.last().unwrap().summary.clone();
    let row = h.get_by_label(sentence).bounding_box().expect("the end row is laid out");
    let b = h.get_by_label(button).bounding_box().expect("the button is laid out");
    let commit = h.get_by_label(&last).bounding_box().expect("the last commit's row");
    assert!(row.y0 >= commit.y1 - 0.5, "the end row {row:?} follows the last commit {commit:?}");
    assert!(b.y0 >= row.y0 && b.y1 <= row.y1, "{button} {b:?} is in the end row {row:?}");
}

/// With every column on, the sentence is cut to the room left and the buttons stay whole.
#[test]
fn graph_end_row_narrow() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "shallow-narrow");
    make_shallow(&mut st);
    st.snap.as_mut().unwrap().truncated = true;
    st.settings.columns.changes = true;
    let mut h = harness_sized(st, true, eframe::egui::vec2(1240.0, 820.0));
    h.run();
    h.check_snapshot("graph_end_row_narrow");
    assert!(h.query_by_label("Load more").is_some());
    assert!(h.query_by_label("Fetch full history").is_some());
    remove_fixture(&dir);
}

#[test]
fn graph_end_row_shallow_no_remote() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "shallow-no-remote");
    make_shallow(&mut st);
    st.snap.as_mut().unwrap().remote_info.clear();
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("graph_shallow_no_remote");
    assert!(h.query_by_label("Fetch full history").is_none());
    let n = h.state().0.snap.as_ref().unwrap().commits.len();
    assert!(h.query_by_label(&format!("Shallow clone: the history stops after {n} commits.")).is_some());
    remove_fixture(&dir);
}

fn deepen_step(query: &str) -> Palette {
    Palette { query: query.into(), mode: Mode::Input(InputKind::DeepenBy), ..Default::default() }
}

#[test]
fn palette_fetch_more_commits() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "palette-deepen");
    make_shallow(&mut st);
    st.palette = Some(deepen_step("20"));
    render("palette_fetch_more_commits", st, true);
    let (mut st, _) = with_repo(ThemeChoice::Dark, "palette-deepen");
    make_shallow(&mut st);
    st.palette = Some(deepen_step("abc"));
    render("palette_fetch_more_commits_invalid", st, true);
    remove_fixture(&dir);
}

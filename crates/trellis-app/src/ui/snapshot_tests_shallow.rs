//! A shallow clone in the graph header (its note and the Fetch full history link, also beside
//! Load more, and without a remote) and the palette's commit count step for fetching more.

use super::snapshot_tests::{harness, remove_fixture, render, with_repo};
use crate::msg::{JobMsg, Msg};
use crate::state::AppState;
use crate::state::palette::{InputKind, Mode, Palette};
use crate::ui::snapshot_check::CheckSnapshot;
use egui_kittest::kittest::Queryable;
use trellis_core::repo::RemoteInfo;
use trellis_core::settings::ThemeChoice;
use trellis_core::shallow_ops::Deepen;

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
fn graph_header_shallow() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "shallow");
    make_shallow(&mut st);
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("graph_shallow");
    assert_note_and_full_link(&h);
    h.get_by_label("Fetch full history").hover();
    h.run();
    h.check_snapshot("graph_shallow_tooltip");
    h.get_by_label("Fetch full history").click();
    h.run();
    assert!(h.state().1.iter().any(|m| matches!(m, Msg::Job(JobMsg::Deepen(Deepen::Full)))));
    remove_fixture(&dir);
}

#[test]
fn graph_header_shallow_and_truncated() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "shallow-truncated");
    make_shallow(&mut st);
    st.snap.as_mut().unwrap().truncated = true;
    st.settings.columns.author = false;
    st.settings.columns.date = false;
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("graph_shallow_load_more_light");
    assert!(h.query_by_label("Load more").is_some());
    assert_note_and_full_link(&h);
    remove_fixture(&dir);
}

/// The "Shallow clone" note shows on the link's line, and the link reads in full: it is wider
/// than the note, which the short "Full history" would not be.
fn assert_note_and_full_link<S>(h: &egui_kittest::Harness<'_, S>) {
    let note = h.get_by_label("Shallow clone").bounding_box().expect("the note is laid out");
    let link = h.get_by_label("Fetch full history").bounding_box().expect("the link is laid out");
    assert!((note.y0 - link.y0).abs() < 1.0, "note {note:?} and link {link:?} share a line");
    assert!(note.x1 < link.x0, "the note sits left of the link");
    assert!(link.width() > note.width(), "the link shows its full text: {link:?} vs {note:?}");
}

/// With every column on, a shallow truncated history's notes still all show, on a line of their
/// own under the titles.
#[test]
fn graph_header_shallow_narrow_message_column() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "shallow-narrow");
    make_shallow(&mut st);
    st.snap.as_mut().unwrap().truncated = true;
    st.settings.columns.changes = true;
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("graph_shallow_notes_line");
    assert!(h.query_by_label("Load more").is_some());
    assert_note_and_full_link(&h);
    remove_fixture(&dir);
}

#[test]
fn graph_header_shallow_no_remote() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "shallow-no-remote");
    make_shallow(&mut st);
    st.snap.as_mut().unwrap().remote_info.clear();
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("graph_shallow_no_remote");
    assert!(h.query_by_label("Fetch full history").is_none());
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

//! Field messages in the shared form style (`widgets/form_message.rs`): a problem with a value
//! nobody typed yet stays hidden until the field is edited or a submit is tried, and
//! information shows from the start.

use super::snapshot_tests::{harness, remove_fixture, with_repo};
use crate::msg::Msg;
use crate::state::touched::Touched;
use crate::state::{AppState, Modal};
use crate::ui::snapshot_check::CheckSnapshot;
use eframe::egui::{Event, Key};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glowfly_core::settings::ThemeChoice;

type H = Harness<'static, (AppState, Vec<Msg>)>;

const INVALID: &str = "Not a valid branch name.";

fn new_branch(name: &str) -> Modal {
    Modal::NewBranch { start: None, name: name.into(), checkout: true, track: None, touched: Touched::default() }
}

/// Types `text` into the focused field.
fn type_text(h: &mut H, text: &str) {
    h.input_mut().events.push(Event::Text(text.into()));
    h.run();
}

#[test]
fn new_branch_untouched() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "form-new-branch-untouched");
    st.modal = Some(new_branch("bad..name"));
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("new_branch_untouched");
    assert!(h.query_by_label(INVALID).is_none(), "nobody typed it yet");
    assert!(h.get_by_label("Create").is_disabled());
    // Enter on the disabled Create shows the problem.
    h.press_key(Key::Enter);
    h.run();
    assert!(h.query_by_label(INVALID).is_some());
    assert!(matches!(&h.state().0.modal, Some(Modal::NewBranch { .. })), "still open");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn new_branch_invalid_typed() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "form-new-branch-typed");
    st.modal = Some(new_branch(""));
    let mut h = harness(st, true);
    h.run();
    assert!(h.query_by_label("Enter a branch name.").is_none(), "an empty name isn't an error yet");
    type_text(&mut h, "bad..name");
    h.check_snapshot("new_branch_invalid_typed");
    assert!(h.query_by_label(INVALID).is_some());
    assert!(h.get_by_label("Create").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn upstream_untouched_invalid() {
    let (mut st, dir) = with_repo(ThemeChoice::Light, "form-upstream-untouched");
    let tip = st.snap.as_ref().unwrap().head_oid.unwrap();
    st.snap.as_mut().unwrap().remotes = vec![("origin".into(), vec![("main".into(), tip)])];
    let touched = Touched::default();
    st.modal = Some(Modal::SetUpstream { branch: "search".into(), remote: "origin".into(), name: "bad..name".into(), touched });
    let mut h = harness(st, false);
    h.run();
    h.check_snapshot("upstream_untouched_invalid");
    assert!(h.query_by_label(INVALID).is_none());
    assert!(h.get_by_label("Save").is_disabled());
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn upstream_tracks_info() {
    let (mut st, dir) = with_repo(ThemeChoice::Dark, "form-upstream-info");
    let tip = st.snap.as_ref().unwrap().head_oid.unwrap();
    st.snap.as_mut().unwrap().remotes = vec![("origin".into(), vec![("main".into(), tip)])];
    let touched = Touched::default();
    st.modal = Some(Modal::SetUpstream { branch: "search".into(), remote: "origin".into(), name: "main".into(), touched });
    let mut h = harness(st, true);
    h.run();
    h.check_snapshot("upstream_tracks_info");
    assert!(h.query_by_label("Tracks origin/main.").is_some(), "information shows untouched");
    drop(h);
    remove_fixture(&dir);
}

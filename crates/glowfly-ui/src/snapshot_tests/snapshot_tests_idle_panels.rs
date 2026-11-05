//! The panels' drafts, drawn from a frame copy, send `Msg::Edit` only when something changed:
//! an idle frame sends none, and typing sends one that lands on the model. Covers the pull
//! request's comment box, the issue's comment box, the rebase panel's message editor and the
//! conflict output editor (the dialogs and Preferences are in `snapshot_tests_idle_forms`).

use super::snapshot_tests::remove_fixture;
use super::snapshot_tests_idle_edits::{H, idle};
use super::snapshot_tests_issue::{crash, crash_details, github, set_details, with_issue};
use super::snapshot_tests_pull_request::with_pr;
use super::snapshot_tests_pull_request_review::{detail, load};
use super::snapshot_tests_rebase::with_panel;
use crate::snapshot_fixtures::edits_taken;
use crate::snapshot_fixtures::with_conflict;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{By, Queryable};
use glowfly_core::rebase::RebaseAction;
use glowfly_core::settings::ThemeChoice;
use glowfly_model::msg::{ConflictMsg, Msg};
use glowfly_model::state::rebase::Editing;
use std::time::Instant;

/// Types into the multi-line text field showing `value` and asserts that sent an edit.
fn typing_edits(h: &mut H, value: &str) {
    h.get(By::new().role(Role::MultilineTextInput).value(value)).type_text("x");
    h.run();
    assert!(edits_taken() > 0, "typing in {value:?} sends an edit");
}

#[test]
fn the_pull_request_comment_idles_and_takes_what_was_typed() {
    let (mut st, dir) = with_pr(ThemeChoice::Dark, "idle-pr", true);
    load(&mut st, detail(), Vec::new());
    st.session.pull_request.view.as_mut().unwrap().forms.comment = "Looks good".into();
    let mut h = idle(st, "pull request");
    typing_edits(&mut h, "Looks good");
    assert_eq!(h.state().0.session.pull_request.view.as_ref().unwrap().forms.comment, "Looks goodx");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_issue_comment_idles_and_takes_what_was_typed() {
    let (mut st, dir) = with_issue(ThemeChoice::Dark, "idle-issue", github(), crash(), None);
    set_details(&mut st, crash_details());
    st.session.issue.view.as_mut().unwrap().comment.text = "Seen it too".into();
    let mut h = idle(st, "issue comment");
    typing_edits(&mut h, "Seen it too");
    assert_eq!(h.state().0.session.issue.view.as_ref().unwrap().comment.text, "Seen it toox");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_rebase_editor_idles_and_takes_what_was_typed() {
    let (mut st, dir) = with_panel(ThemeChoice::Dark, "idle-rebase", &[RebaseAction::Reword, RebaseAction::Pick, RebaseAction::Pick]);
    let p = st.session.rebase.view.as_mut().unwrap();
    p.editing = Some(Editing::Message(0));
    p.draft = "Reword it".into();
    let mut h = idle(st, "rebase editor");
    typing_edits(&mut h, "Reword it");
    assert_eq!(h.state().0.session.rebase.view.as_ref().unwrap().draft, "Reword itx");
    drop(h);
    remove_fixture(&dir);
}

#[test]
fn the_conflict_output_idles_and_takes_what_was_typed() {
    let (mut st, dir) = with_conflict("idle-conflict", "notes.txt", |side| format!("{side}\n").into_bytes());
    glowfly_model::update::update(&mut st, Msg::Conflict(ConflictMsg::ConflictEditOutput(true)), Instant::now());
    let shown = st.session.conflict.view.as_ref().unwrap().edit.as_ref().expect("editing").buffer.clone();
    let mut h = idle(st, "conflict output");
    typing_edits(&mut h, &shown);
    let edit = h.state().0.session.conflict.view.as_ref().unwrap().edit.as_ref().map(|e| e.buffer.clone());
    assert_ne!(edit.as_deref(), Some(shown.as_str()), "the buffer took the text");
    drop(h);
    remove_fixture(&dir);
}

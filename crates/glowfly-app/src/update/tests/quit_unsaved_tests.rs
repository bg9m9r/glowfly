//! Headless tests for work that exists only in memory (#188): quitting asks while any tab holds
//! a commit message draft, conflict choices or comments not posted, an edited dialog is open or
//! a Preferences page has unsaved edits, in one question with what runs; the question never
//! closes the window by itself while it names such work; an edited dialog stays under the quit
//! question, a tab close's confirmation and the trust prompt, and comes back when they close;
//! closing a tab that holds an edited dialog or comments not posted asks first.

use super::prefs_save_tests::{git_mut, identity};
use super::quit_tests::pushing;
use super::tabs_close_tests::{conflict, fake_tabs, names, send};
use super::trust_held_tests::held_repo;
use super::update;
use crate::effect::{Effect, UiEffect};
use crate::msg::{AppMsg, DialogMsg, Dismiss, JobMsg, Msg, TabsMsg};
use crate::state::dialog_guard::edited_form;
use crate::state::session::RepoSession;
use crate::state::tabs::{Parked, RenameTabForm};
use crate::state::{AppState, CommitDraft, Modal};
use glowfly_core::conflict_pick::ChunkPick;
use glowfly_core::repo::Choice;
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

fn quit(st: &mut AppState) -> Vec<Effect> {
    run(st, Msg::App(AppMsg::Quit))
}

fn closes(fx: &[Effect]) -> bool {
    fx.iter().any(|e| matches!(e, Effect::Ui(UiEffect::CloseWindow)))
}

/// A message no handler acts on, to end an update.
fn nothing() -> Msg {
    Msg::Logged { ok: true, text: "-".into() }
}

/// The open danger confirmation's title and body.
fn question(st: &AppState) -> (&str, &str) {
    match &st.modal {
        Some(Modal::ConfirmDanger { title, body, .. }) => (title, body),
        _ => panic!("no question"),
    }
}

const HEADER: &str = "Quitting loses work that isn't saved anywhere:";

/// Opens the Rename tab dialog for the active tab (recorded as it opened), then types `text`.
fn edited_rename(st: &mut AppState, text: &str) {
    let i = st.active_tab;
    st.modal = Some(Modal::RenameTab(RenameTabForm::new(&st.tabs[i])));
    run(st, nothing());
    if let Some(Modal::RenameTab(f)) = st.modal.as_mut() {
        f.text = text.into();
    }
}

fn rename_text(st: &AppState) -> Option<&str> {
    match &st.modal {
        Some(Modal::RenameTab(f)) => Some(&f.text),
        _ => None,
    }
}

fn draft(summary: &str) -> CommitDraft {
    CommitDraft { summary: summary.into(), ..Default::default() }
}

#[test]
fn quitting_with_a_draft_asks_and_names_its_tab() {
    let mut st = fake_tabs(&["a", "b"], 0);
    let session = RepoSession { draft: draft("wip"), ..Default::default() };
    st.tabs[1].parked = Some(Box::new(Parked { session, ..Default::default() }));
    let fx = quit(&mut st);
    assert!(!closes(&fx), "{fx:?}");
    let (title, body) = question(&st);
    assert_eq!(title, "Quit Glowfly?");
    assert_eq!(body, format!("{HEADER}\n- b: its commit message draft"));
    assert!(!st.view.quitting);
    // The OS's close is cancelled too.
    run(&mut st, Msg::CloseModal);
    let fx = run(&mut st, Msg::App(AppMsg::CloseRequested));
    assert!(fx.iter().any(|e| matches!(e, Effect::Ui(UiEffect::CancelClose))), "{fx:?}");
    assert!(!st.view.quitting);
    // Quit anyway closes.
    assert!(closes(&run(&mut st, Msg::ConfirmAccepted)));
    assert!(st.view.quitting);
}

#[test]
fn quitting_with_conflict_choices_asks() {
    let mut st = fake_tabs(&["a"], 0);
    let picks = vec![ChunkPick::Sides(Choice::Ours), ChunkPick::Sides(Choice::Theirs)];
    st.session.conflict.view = Some(conflict(picks, vec![None, None]));
    assert!(!closes(&quit(&mut st)));
    assert_eq!(question(&st).1, format!("{HEADER}\n- 2 conflict choices in src/lib.rs"));
}

#[test]
fn quitting_with_an_edited_dialog_asks_and_keeps_it_underneath() {
    let mut st = fake_tabs(&["a"], 0);
    edited_rename(&mut st, "api");
    assert!(!closes(&quit(&mut st)));
    assert_eq!(question(&st).1, format!("{HEADER}\n- the changes in the open dialog"));
    assert!(matches!(st.dialog.under(), Some(Modal::RenameTab(f)) if f.text == "api"), "the form waits underneath");
    // Cancel: the form is back as it was, and still counts as edited (Esc asks first).
    run(&mut st, Msg::CloseModal);
    assert_eq!(rename_text(&st), Some("api"));
    assert!(st.dialog.under().is_none());
    assert!(edited_form(&st.dialog, st.modal.as_ref()));
    run(&mut st, Msg::Dialog(DialogMsg::Dismiss(Dismiss::Escape)));
    assert!(st.dialog.asking, "Esc still asks before discarding it");
}

#[test]
fn quitting_with_an_unsaved_preferences_page_asks() {
    let mut st = identity();
    git_mut(&mut st).repo_name = "Grace".into();
    assert!(!closes(&quit(&mut st)));
    let body = question(&st).1;
    assert!(body.starts_with(HEADER), "{body}");
    assert!(body.ends_with("\n- Preferences: the changes on the Identity page that aren't saved yet"), "{body}");
}

#[test]
fn quitting_with_nothing_unsaved_closes() {
    let mut st = fake_tabs(&["a", "b"], 0);
    st.session.draft.summary = "  ".into();
    // An unedited dialog loses nothing.
    let i = st.active_tab;
    st.modal = Some(Modal::RenameTab(RenameTabForm::new(&st.tabs[i])));
    run(&mut st, nothing());
    let fx = quit(&mut st);
    assert!(closes(&fx), "{fx:?}");
    assert!(st.view.quitting);
    // An unedited Preferences page closes too.
    let mut st = identity();
    assert!(closes(&quit(&mut st)));
}

#[test]
fn one_question_names_unsaved_and_running_work_and_never_closes_by_itself() {
    let mut st = pushing();
    let api = st.tabs.len() - 1;
    st.tabs[api].parked =
        Some(Box::new(Parked { session: RepoSession { draft: draft("wip"), ..Default::default() }, ..Default::default() }));
    quit(&mut st);
    let risk = "Quitting now can cut it off partway, and Glowfly won't see how it ends.";
    let lost = format!("{HEADER}\n- api: its commit message draft");
    assert_eq!(question(&st).1, format!("{lost}\n\nPush is still running in api.\n\n{risk}"));
    // The push ends: the draft is still there, so the question stays, naming only the draft.
    let fx = run(&mut st, Msg::Job(JobMsg::JobDone { ok: true, output: String::new(), undo: None }));
    assert!(!closes(&fx), "{fx:?}");
    assert!(!st.view.quitting);
    assert_eq!(question(&st).1, lost);
    assert!(!closes(&run(&mut st, nothing())));
    assert!(crate::state::quit_confirm::asking(&st));
}

#[test]
fn the_question_is_translated() {
    let mut st = fake_tabs(&["a", "b"], 0);
    st.settings.language = "de".into();
    st.session.draft.summary = "wip".into();
    quit(&mut st);
    let (title, body) = question(&st);
    assert_eq!(title, "Glowfly beenden?");
    assert_eq!(body, "Beim Beenden geht Arbeit verloren, die nirgends gespeichert ist:\n- a: der Entwurf der Commit-Nachricht");
}

#[test]
fn closing_a_tab_with_an_edited_dialog_asks_and_keeps_it_on_cancel() {
    let mut st = fake_tabs(&["a", "b"], 1);
    edited_rename(&mut st, "api");
    send(&mut st, TabsMsg::CloseTab(1));
    let (title, body) = question(&st);
    assert_eq!(title, "Close tab?");
    assert_eq!(body, "b holds work that isn't saved anywhere:\n- the changes in the open dialog");
    run(&mut st, Msg::CloseModal);
    assert_eq!(rename_text(&st), Some("api"), "Cancel brings the form back");
    assert_eq!(names(&st), ["a", "b"]);
    // Accepted: the tab closes, and its form with it.
    send(&mut st, TabsMsg::CloseTab(1));
    run(&mut st, Msg::ConfirmAccepted);
    assert_eq!(names(&st), ["a"]);
    assert!(st.modal.is_none() && st.dialog.under().is_none());
}

#[test]
fn closing_a_tab_with_a_comment_not_posted_asks() {
    let (mut st, dir) = super::issue_comment_tests::loaded_panel("unsaved-comment");
    st.session.issue.view.as_mut().expect("the panel").comment.text = "Looks good".into();
    send(&mut st, TabsMsg::CloseTab(usize::MAX));
    let tab = crate::state::menu_bar::tab_name(&st, st.active_tab).to_owned();
    assert_eq!(
        question(&st).1,
        format!(
            "{tab} holds work that isn't saved anywhere:
- comments typed and not posted yet"
        )
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_trust_prompt_keeps_an_edited_form_underneath() {
    let (mut st, dir) = held_repo("unsaved-trust");
    edited_rename(&mut st, "api");
    run(&mut st, Msg::git("Pull", &["pull", "--ff"]));
    assert!(matches!(st.modal, Some(Modal::Trust(_))), "the prompt shows");
    assert!(st.dialog.under().is_some(), "the form waits underneath");
    run(&mut st, Msg::CloseModal);
    assert_eq!(rename_text(&st), Some("api"));
    let _ = std::fs::remove_dir_all(&dir);
}

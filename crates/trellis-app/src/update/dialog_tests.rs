//! Headless tests for dismissing dialogs: a form ignores the backdrop, Esc and the close button
//! close it at once only while it matches how it opened, and otherwise ask "Discard your
//! changes?" first; information and confirm dialogs close on all three.

use super::tests::state;
use super::update;
use crate::msg::{AppMsg, BranchMsg, CloneMsg, DialogMsg, Dismiss, InitMsg, Msg, RepoMsg};
use crate::state::askpass::PendingPrompt;
use crate::state::{AppState, Modal};
use std::time::Instant;

fn run(st: &mut AppState, msg: Msg) -> Vec<crate::effect::Effect> {
    update(st, msg, Instant::now())
}

fn dismiss(st: &mut AppState, how: Dismiss) {
    run(st, Msg::Dialog(DialogMsg::Dismiss(how)));
}

fn new_branch(st: &mut AppState, name: &str) {
    run(st, Msg::Branch(BranchMsg::NewBranch { start: None, name: name.into() }));
}

/// The New branch dialog's name; panics when another dialog (or none) is open.
fn branch_name(st: &mut AppState) -> &mut String {
    match st.modal.as_mut() {
        Some(Modal::NewBranch { name, .. }) => name,
        _ => panic!("the New branch dialog isn't open"),
    }
}

fn clone_url(st: &mut AppState) -> &mut String {
    match st.modal.as_mut() {
        Some(Modal::Clone(f)) => &mut f.url,
        _ => panic!("the Clone dialog isn't open"),
    }
}

#[test]
fn backdrop_never_closes_a_form() {
    let mut st = state();
    new_branch(&mut st, "");
    *branch_name(&mut st) = "wip".into();
    dismiss(&mut st, Dismiss::Backdrop);
    assert_eq!(branch_name(&mut st), "wip", "still open, still holding what was typed");
    assert!(!st.dialog.asking);

    // Unedited too.
    let mut st = state();
    new_branch(&mut st, "");
    dismiss(&mut st, Dismiss::Backdrop);
    assert!(matches!(st.modal, Some(Modal::NewBranch { .. })));
    assert!(!st.dialog.asking);
}

#[test]
fn esc_closes_an_unedited_form_even_when_prefilled() {
    let mut st = state();
    new_branch(&mut st, "feature/x");
    dismiss(&mut st, Dismiss::Escape);
    assert!(st.modal.is_none());
    assert!(!st.dialog.asking);
}

#[test]
fn esc_on_an_edited_form_asks_and_keep_editing_keeps_it() {
    let url = "https://example.com/r.git";
    let mut st = state();
    run(&mut st, Msg::Clone(CloneMsg::Dialog));
    *clone_url(&mut st) = url.into();
    dismiss(&mut st, Dismiss::Escape);
    assert!(st.dialog.asking, "asks first");
    assert_eq!(clone_url(&mut st), url);

    run(&mut st, Msg::Dialog(DialogMsg::KeepEditing));
    assert!(!st.dialog.asking);
    assert_eq!(clone_url(&mut st), url, "back to the form, unchanged");

    dismiss(&mut st, Dismiss::Escape);
    assert!(st.dialog.asking, "asks again");
    // Esc on the question means Keep editing, so a second Esc never loses the text.
    dismiss(&mut st, Dismiss::Escape);
    assert!(!st.dialog.asking);
    assert_eq!(clone_url(&mut st), url);
    // So does a click on its backdrop.
    dismiss(&mut st, Dismiss::Escape);
    dismiss(&mut st, Dismiss::Backdrop);
    assert!(st.dialog.asking, "the backdrop doesn't touch a form or the question over it");

    run(&mut st, Msg::Dialog(DialogMsg::Discard));
    assert!(st.modal.is_none());
    assert!(!st.dialog.asking);

    run(&mut st, Msg::Clone(CloneMsg::Dialog));
    assert_eq!(clone_url(&mut st), "", "reopening starts fresh");
    dismiss(&mut st, Dismiss::Escape);
    assert!(st.modal.is_none(), "and counts as unedited");
}

#[test]
fn editing_back_to_the_opening_value_is_unedited() {
    let mut st = state();
    new_branch(&mut st, "feature/x");
    *branch_name(&mut st) = "feature/xy".into();
    run(&mut st, Msg::Tick(Instant::now()));
    *branch_name(&mut st) = "feature/x".into();
    dismiss(&mut st, Dismiss::Escape);
    assert!(st.modal.is_none(), "compared by value");
    assert!(!st.dialog.asking);
}

#[test]
fn a_result_filling_a_field_is_not_an_edit() {
    let mut st = state();
    run(&mut st, Msg::Repo(RepoMsg::InitRepo));
    run(&mut st, Msg::Init(InitMsg::AuthorRead(Some("Ada".into()))));
    assert!(matches!(&st.modal, Some(Modal::Init(f)) if f.holder == "Ada"));
    dismiss(&mut st, Dismiss::Escape);
    assert!(st.modal.is_none(), "the suggested holder isn't an edit");

    let mut st = state();
    run(&mut st, Msg::Repo(RepoMsg::InitRepo));
    run(&mut st, Msg::Init(InitMsg::AuthorRead(Some("Ada".into()))));
    if let Some(Modal::Init(f)) = st.modal.as_mut() {
        f.holder = "Ada Lovelace".into();
    }
    dismiss(&mut st, Dismiss::Escape);
    assert!(st.dialog.asking, "a holder the user typed is");
    assert!(matches!(st.modal, Some(Modal::Init(_))));
}

#[test]
fn info_and_confirm_dialogs_close_on_backdrop_and_esc() {
    let mut st = state();
    run(&mut st, Msg::App(AppMsg::ShowAbout));
    assert!(matches!(st.modal, Some(Modal::About)));
    dismiss(&mut st, Dismiss::Backdrop);
    assert!(st.modal.is_none());

    // A confirm closes without running what it asked about.
    let mut st = state();
    st.log(true, "kept");
    run(&mut st, Msg::confirm("T", "B".into(), "Go", Msg::ClearLog));
    dismiss(&mut st, Dismiss::Backdrop);
    assert!(st.modal.is_none());
    assert!(!st.log.is_empty(), "`then` didn't run");
    run(&mut st, Msg::confirm("T", "B".into(), "Go", Msg::ClearLog));
    dismiss(&mut st, Dismiss::Escape);
    assert!(st.modal.is_none());

    // The shortcut sheet's filter is cheap to type again.
    let mut st = state();
    st.modal = Some(Modal::Shortcuts { filter: String::new() });
    run(&mut st, Msg::Tick(Instant::now()));
    st.modal = Some(Modal::Shortcuts { filter: "stage".into() });
    dismiss(&mut st, Dismiss::Escape);
    assert!(st.modal.is_none());
    assert!(!st.dialog.asking);
}

#[test]
fn closing_or_replacing_the_dialog_drops_the_question() {
    let mut st = state();
    new_branch(&mut st, "");
    *branch_name(&mut st) = "wip".into();
    dismiss(&mut st, Dismiss::Escape);
    assert!(st.dialog.asking);
    run(&mut st, Msg::CloseModal);
    assert!(st.modal.is_none());
    assert!(!st.dialog.asking);

    // A dialog that replaces the form takes the question away, and is recorded afresh.
    let mut st = state();
    new_branch(&mut st, "");
    *branch_name(&mut st) = "wip".into();
    dismiss(&mut st, Dismiss::Escape);
    assert!(st.dialog.asking);
    run(&mut st, Msg::confirm("T", "B".into(), "Go", Msg::ClearLog));
    assert!(matches!(st.modal, Some(Modal::Confirm { .. })));
    assert!(!st.dialog.asking);
    run(&mut st, Msg::CloseModal);
    new_branch(&mut st, "main-2");
    assert!(!st.dialog.edited(st.modal.as_ref().unwrap()), "the new form's draft is its own");
    dismiss(&mut st, Dismiss::Escape);
    assert!(st.modal.is_none());
}

#[test]
fn dismiss_messages_without_a_dialog_do_nothing() {
    let mut st = state();
    for m in [
        DialogMsg::Dismiss(Dismiss::Escape),
        DialogMsg::Dismiss(Dismiss::Backdrop),
        DialogMsg::Dismiss(Dismiss::CloseButton),
        DialogMsg::KeepEditing,
        DialogMsg::Discard,
    ] {
        assert!(m.on_inactive().is_none());
        assert!(run(&mut st, Msg::Dialog(m)).is_empty());
        assert!(st.modal.is_none());
        assert!(!st.dialog.asking);
    }
}

#[test]
fn a_credential_prompt_keeps_esc_and_clicks() {
    let mut st = state();
    run(&mut st, Msg::App(AppMsg::ShowAbout));
    let prompt = trellis_core::askpass::Prompt::new("Password: ", None);
    st.askpass.queue.push_back(PendingPrompt { id: 1, prompt, retry: false, rejected_before: false });
    dismiss(&mut st, Dismiss::Escape);
    dismiss(&mut st, Dismiss::Backdrop);
    assert!(matches!(st.modal, Some(Modal::About)), "nothing is dismissed under the prompt");
}

fn copies(fx: &[crate::effect::Effect], text: &str) -> bool {
    use crate::effect::{Effect, UiEffect};
    matches!(fx, [Effect::Ui(UiEffect::CopyText(t))] if t == text)
}

#[test]
fn danger_confirmation_waits_for_its_acknowledgement() {
    let mut st = state();
    run(&mut st, Msg::confirm_danger_ack("Delete", "Sure?".into(), "Delete", "I understand", Msg::Copy("x".into())));
    assert!(matches!(&st.modal, Some(Modal::ConfirmDanger { ack: Some(a), acked: false, .. }) if a == "I understand"));
    assert!(run(&mut st, Msg::ConfirmAccepted).is_empty(), "nothing runs before the tick");
    assert!(matches!(st.modal, Some(Modal::ConfirmDanger { .. })), "the question stays open");
    if let Some(Modal::ConfirmDanger { acked, .. }) = st.modal.as_mut() {
        *acked = true;
    }
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert!(st.modal.is_none());
    assert!(copies(&fx, "x"));
}

#[test]
fn danger_confirmation_without_acknowledgement_runs_at_once() {
    let mut st = state();
    run(&mut st, Msg::confirm_danger("Delete", "Sure?".into(), "Delete", Msg::Copy("y".into())));
    assert!(matches!(st.modal, Some(Modal::ConfirmDanger { ack: None, acked: false, .. })));
    let fx = run(&mut st, Msg::ConfirmAccepted);
    assert!(st.modal.is_none());
    assert!(copies(&fx, "y"));
}

#[test]
fn close_button_closes_an_unedited_form() {
    let mut st = state();
    new_branch(&mut st, "feature/x");
    dismiss(&mut st, Dismiss::CloseButton);
    assert!(st.modal.is_none());
    assert!(!st.dialog.asking);
}

#[test]
fn close_button_asks_on_an_edited_form() {
    let mut st = state();
    new_branch(&mut st, "");
    *branch_name(&mut st) = "wip".into();
    dismiss(&mut st, Dismiss::CloseButton);
    assert!(st.dialog.asking, "asks first");
    assert_eq!(branch_name(&mut st), "wip");
    // On the question it means Keep editing, as Esc does.
    dismiss(&mut st, Dismiss::CloseButton);
    assert!(!st.dialog.asking);
    assert_eq!(branch_name(&mut st), "wip", "still open");
}

#[test]
fn close_button_closes_confirm_and_info() {
    let mut st = state();
    st.log(true, "kept");
    run(&mut st, Msg::confirm("T", "B".into(), "Go", Msg::ClearLog));
    assert!(matches!(st.modal, Some(Modal::Confirm { .. })));
    dismiss(&mut st, Dismiss::CloseButton);
    assert!(st.modal.is_none());
    assert!(!st.log.is_empty(), "`then` didn't run");

    run(&mut st, Msg::App(AppMsg::ShowAbout));
    assert!(matches!(st.modal, Some(Modal::About)));
    dismiss(&mut st, Dismiss::CloseButton);
    assert!(st.modal.is_none());
}

#[test]
fn close_button_on_trust_prompt_drops_the_job() {
    let (mut st, dir) = super::trust_held_tests::held_repo("close-button-trust");
    run(&mut st, Msg::git("Pull", &["pull", "--ff"]));
    assert!(matches!(st.modal, Some(Modal::Trust(_))));
    assert!(st.trust.held.is_some());
    dismiss(&mut st, Dismiss::CloseButton);
    assert!(st.modal.is_none());
    assert!(st.trust.held.is_none(), "the held job is dropped");
    assert!(st.log.iter().any(|e| e.text.contains("Pull wasn't run")));
    let _ = std::fs::remove_dir_all(&dir);
}

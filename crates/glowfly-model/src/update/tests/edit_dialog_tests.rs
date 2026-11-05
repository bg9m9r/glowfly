//! Headless tests for the dialogs' edits (update/edit.rs, state/modal_form.rs): a form's copy
//! lands on the open dialog of the same kind only, a form kept under a question is never
//! touched, the danger confirmation's and auto-stash question's boxes, Generate SSH key's
//! fields, and a key handled in the same frame seeing the typed text.

use super::tests::state;
use super::update;
use crate::effect::{CloneEffect, Effect};
use crate::msg::{CloneMsg, EditMsg, Msg, edits_first};
use crate::state::auto_stash::AutoStashAsk;
use crate::state::clone::CloneForm;
use crate::state::dialog_guard;
use crate::state::modal_form::DialogForm;
use crate::state::ssh::KeygenFields;
use crate::state::tabs::{RenameTabForm, Tab};
use crate::state::tags::TagForm;
use crate::state::{AppState, Modal, PrefPage, PrefsState};
use glowfly_core::stash_ops::AutoStashFor;
use std::time::Instant;

fn edit(st: &mut AppState, e: EditMsg) -> Vec<Effect> {
    update(st, Msg::Edit(e), Instant::now())
}

fn dialog(form: Modal) -> EditMsg {
    EditMsg::Dialog(DialogForm::new(form))
}

fn tag(name: &str) -> Modal {
    Modal::NewTag(TagForm { at: "abc".into(), name: name.into(), ..Default::default() })
}

fn rename_tab(text: &str) -> Modal {
    let mut form = RenameTabForm::new(&Tab::unloaded(glowfly_core::paths::temp_dir().join("glowfly-edit-dialog")));
    form.text = text.into();
    Modal::RenameTab(form)
}

fn new_branch() -> Modal {
    Modal::NewBranch { start: None, name: String::new(), checkout: true, track: None, touched: Default::default() }
}

#[test]
fn a_form_edit_lands_on_the_same_dialog_only() {
    let mut st = state();
    st.modal = Some(tag(""));
    edit(&mut st, dialog(tag("v1.0")));
    assert!(matches!(&st.modal, Some(Modal::NewTag(f)) if f.name == "v1.0"));

    st.modal = Some(rename_tab("mine"));
    edit(&mut st, dialog(tag("v2.0")));
    assert!(matches!(&st.modal, Some(Modal::RenameTab(f)) if f.text == "mine"), "another kind of dialog is open");

    st.modal = None;
    edit(&mut st, dialog(tag("v3.0")));
    assert!(st.modal.is_none(), "no dialog: dropped");
}

#[test]
fn forms_copy_and_compare_but_questions_and_notes_do_not() {
    assert!(new_branch().form_copy().is_some());
    assert!(Modal::About.form_copy().is_none());
    assert!(Modal::danger("Drop?", "Gone.", "Drop", Msg::ClearLog).form_copy().is_none());
    let ask = AutoStashAsk { op: AutoStashFor::Pull { branch: "main".into() }, files: 1, always: false, retry: Box::new(Msg::ClearLog) };
    assert!(Modal::AutoStash(ask).form_copy().is_none());
    let confirm = Modal::Confirm { title: "t".into(), body: "b".into(), button: "OK".into(), then: Box::new(Msg::ClearLog) };
    assert!(confirm.form_copy().is_none());

    let shown = new_branch();
    let mut copy = shown.form_copy().expect("a form");
    assert!(shown.same_form(&copy), "an untouched copy");
    if let Modal::NewBranch { name, .. } = &mut copy {
        *name = "feature".into();
    }
    assert!(!shown.same_form(&copy), "typed in");
    assert!(!Modal::About.same_form(&Modal::About), "not a form");

    let mut open = tag("a");
    assert!(!open.take_form(rename_tab("x")), "another variant is refused");
    assert!(matches!(&open, Modal::NewTag(f) if f.name == "a"));
    assert!(open.take_form(tag("b")));
    assert!(matches!(&open, Modal::NewTag(f) if f.name == "b"));
    let printed = format!("{:?}", DialogForm::new(tag("secret-name")));
    assert!(!printed.contains("secret-name"), "Debug shows no form contents: {printed}");
}

/// Every kind of form (struct-like, one-field, filter) compares equal to its untouched copy and
/// unequal to another dialog, so an idle dialog sends no edit (`same_form` has no catch-all).
#[test]
fn each_form_matches_its_own_copy_and_no_other_dialog() {
    let upstream = Modal::SetUpstream { branch: "main".into(), remote: "origin".into(), name: "main".into(), touched: Default::default() };
    let forms = [
        new_branch(),
        upstream,
        tag("v1"),
        rename_tab("tab"),
        Modal::Shortcuts { filter: "f".into() },
        Modal::FindRepos { filter: "f".into() },
    ];
    for (i, form) in forms.iter().enumerate() {
        let copy = form.form_copy().expect("a form");
        assert!(form.same_form(&copy), "form {i} against its copy");
        for (j, other) in forms.iter().enumerate().filter(|&(j, _)| j != i) {
            assert!(!form.same_form(other), "form {i} against form {j}");
        }
        assert!(!form.same_form(&Modal::About), "form {i} against a note");
        assert!(!Modal::About.same_form(form), "a note against form {i}");
    }
}

#[test]
fn a_parent_typed_with_its_check_in_one_frame_is_the_one_checked() {
    let mut st = state();
    st.modal = Some(Modal::Clone(CloneForm::new(String::new())));
    let typed = glowfly_core::paths::temp_dir().join("glowfly-clone-parent").display().to_string();
    let Some(Modal::Clone(mut form)) = st.modal.as_ref().and_then(Modal::form_copy) else { panic!("the Clone form") };
    form.parent = typed.clone();
    // The view pushes the check first and its edit after; the shell runs the edit first.
    let mut msgs = vec![Msg::Clone(CloneMsg::ParentEdited), Msg::Edit(dialog(Modal::Clone(form)))];
    edits_first(&mut msgs);
    let fx: Vec<Effect> = msgs.into_iter().flat_map(|m| update(&mut st, m, Instant::now())).collect();
    let checked: Vec<&str> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::Clone(CloneEffect::CheckParent { parent, .. }) => Some(parent.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(checked, [typed.as_str()]);
}

#[test]
fn a_form_kept_under_a_question_is_never_edited() {
    let mut st = state();
    st.modal = Some(rename_tab(""));
    update(&mut st, Msg::ClearLog, Instant::now());
    edit(&mut st, dialog(rename_tab("typed")));
    assert!(dialog_guard::edited_form(&st.dialog, st.modal.as_ref()), "the form counts as edited");
    let question = Modal::danger("Quit?", "Work is running.", "Quit", Msg::ClearLog).with_ack("I understand");
    dialog_guard::open_over(&mut st.modal, &mut st.dialog, question);
    update(&mut st, Msg::ClearLog, Instant::now());

    edit(&mut st, dialog(rename_tab("over")));
    edit(&mut st, EditMsg::ConfirmAck(true));
    assert!(matches!(&st.modal, Some(Modal::ConfirmDanger { acked: true, .. })), "the question takes its own edit");
    assert!(matches!(st.dialog.under(), Some(Modal::RenameTab(f)) if f.text == "typed"), "the form underneath keeps its text");

    update(&mut st, Msg::CloseModal, Instant::now());
    assert!(matches!(&st.modal, Some(Modal::RenameTab(f)) if f.text == "typed"), "back as it was");
}

#[test]
fn the_ack_and_always_boxes_land_on_their_own_question() {
    let mut st = state();
    st.modal = Some(Modal::danger("Drop?", "Gone.", "Drop", Msg::ClearLog).with_ack("I understand"));
    edit(&mut st, EditMsg::ConfirmAck(true));
    assert!(matches!(&st.modal, Some(Modal::ConfirmDanger { acked: true, .. })));
    assert!(st.modal.as_ref().is_some_and(Modal::confirm_ready));

    st.modal = Some(Modal::Confirm { title: "t".into(), body: "b".into(), button: "OK".into(), then: Box::new(Msg::ClearLog) });
    edit(&mut st, EditMsg::ConfirmAck(true));
    edit(&mut st, EditMsg::AutoStashAlways(true));
    assert!(matches!(&st.modal, Some(Modal::Confirm { .. })), "a plain confirmation has neither box");

    let ask = AutoStashAsk { op: AutoStashFor::Pull { branch: "main".into() }, files: 2, always: false, retry: Box::new(Msg::ClearLog) };
    st.modal = Some(Modal::AutoStash(ask));
    edit(&mut st, EditMsg::AutoStashAlways(true));
    assert!(matches!(&st.modal, Some(Modal::AutoStash(a)) if a.always && a.files == 2));
}

#[test]
fn keygen_fields_land_while_preferences_is_open() {
    let mut st = state();
    let fields = KeygenFields { location: "~/.ssh/work".into(), comment: "ada".into(), touched: Default::default() };
    edit(&mut st, EditMsg::SshKeygen(fields.clone()));
    assert!(st.prefs.is_none(), "dropped with Preferences closed");

    st.prefs = Some(PrefsState::new(PrefPage::Ssh));
    edit(&mut st, EditMsg::SshKeygen(fields.clone()));
    let ssh = &st.prefs.as_ref().expect("open").ssh;
    assert_eq!(ssh.keygen_fields(), fields);
}

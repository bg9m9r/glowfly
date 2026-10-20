//! Headless tests for the Generate SSH key dialog: Generate… opens it with the location and
//! comment filled, its fields outlive it, Esc closes it without the discard question, and a
//! failed ssh-keygen run shows in the dialog or, with the dialog closed, as a toast.

use super::ssh_tests::{email, fill, home, loaded, page, page_mut, run, ssh};
use super::tests::state;
use crate::msg::{DialogMsg, Dismiss, Msg, PrefsMsg};
use crate::ssh::{GenerateFailure, SshMsg};
use crate::state::{Modal, PrefPage};
use trellis_core::ssh_keys::default_location;

#[test]
fn open_generate_opens_the_dialog() {
    let mut st = loaded(vec![], true);
    email(&mut st, "ada@example.com");
    page_mut(&mut st).location.clear();
    page_mut(&mut st).comment.clear();
    page_mut(&mut st).comment_prefilled = false;
    ssh(&mut st, SshMsg::OpenGenerate);
    assert!(matches!(st.modal, Some(Modal::SshKeygen)));
    assert_eq!(page(&st).location, default_location(&home()).display().to_string());
    assert_eq!(page(&st).comment, "ada@example.com", "prefilled from user.email");

    // Closing keeps what was typed; opening again shows it.
    fill(&mut st, "~/.ssh/work", "ada", "s3cret", "s3cret");
    run(&mut st, Msg::CloseModal);
    assert!(st.modal.is_none());
    ssh(&mut st, SshMsg::OpenGenerate);
    assert!(matches!(st.modal, Some(Modal::SshKeygen)));
    assert_eq!((page(&st).location.as_str(), page(&st).passphrase.as_str()), ("~/.ssh/work", "s3cret"));

    let mut st = loaded(vec![], false);
    ssh(&mut st, SshMsg::OpenGenerate);
    assert!(st.modal.is_none(), "no ssh-keygen");
    assert_eq!(page(&st).form_error.as_deref(), Some("ssh-keygen wasn't found."));

    let mut st = state();
    run(&mut st, Msg::Prefs(PrefsMsg::OpenPrefs(PrefPage::Ssh)));
    ssh(&mut st, SshMsg::OpenGenerate);
    assert!(st.modal.is_none(), "the folder is still being read");
}

#[test]
fn esc_closes_the_keygen_dialog_without_asking() {
    let mut st = loaded(vec![], true);
    ssh(&mut st, SshMsg::OpenGenerate);
    fill(&mut st, "~/.ssh/work", "ada", "s3cret", "s3cret");
    run(&mut st, Msg::Dialog(DialogMsg::Dismiss(Dismiss::Backdrop)));
    assert!(matches!(st.modal, Some(Modal::SshKeygen)), "a backdrop click does nothing");
    run(&mut st, Msg::Dialog(DialogMsg::Dismiss(Dismiss::Escape)));
    assert!(st.modal.is_none());
    assert!(!st.dialog.asking, "nothing is lost, so nothing to ask");
    assert_eq!(page(&st).location, "~/.ssh/work");
}

#[test]
fn generate_failure_without_the_dialog_toasts() {
    let mut st = loaded(vec![], true);
    let failed = || SshMsg::Generated(Err(GenerateFailure::Failed("ssh-keygen failed: boom".into())));
    ssh(&mut st, failed());
    let toast = st.toasts.last().expect("a toast");
    assert!(!toast.ok && toast.text.contains("ssh-keygen failed: boom"), "{}", toast.text);
    assert!(st.log.last().is_some_and(|e| !e.ok && e.text.starts_with("Could not generate an SSH key")));

    let mut st = loaded(vec![], true);
    ssh(&mut st, SshMsg::OpenGenerate);
    let toasts = st.toasts.len();
    ssh(&mut st, failed());
    assert_eq!(page(&st).form_error.as_deref(), Some("ssh-keygen failed: boom"), "shown in the dialog");
    assert_eq!(st.toasts.len(), toasts, "no toast while the dialog shows it");
    assert!(matches!(st.modal, Some(Modal::SshKeygen)));
}

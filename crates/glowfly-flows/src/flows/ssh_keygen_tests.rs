//! Headless tests for the Generate SSH key dialog: Generate… opens it with the location and
//! comment filled, its fields outlive it, Esc closes it without the discard question, and a
//! failed ssh-keygen run shows in the dialog or, with the dialog closed, as a toast, and the
//! reasons Generate is refused are in the chosen language.

use super::ssh_tests::{email, fill, home, loaded, page, page_mut, run, ssh};
use super::tests::state;
use crate::msg::{DialogMsg, Dismiss, Msg, PrefsMsg};
use crate::ssh::{GenerateFailure, SshMsg};
use crate::state::ssh::location_text;
use crate::state::{Modal, PrefPage};
use glowfly_core::i18n::Lang;
use glowfly_core::ssh_keys::{LocationProblem, default_location};

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

#[test]
fn refusals_follow_the_language() {
    let de = Lang::De;
    let mut st = loaded(vec![], false);
    st.settings.language = "de".into();
    ssh(&mut st, SshMsg::OpenGenerate);
    assert!(st.modal.is_none(), "no ssh-keygen");
    let missing = de.tr("prefs.ssh.generate.no_keygen.warn");
    assert_ne!(missing, Lang::En.tr("prefs.ssh.generate.no_keygen.warn"), "German has its own text");
    assert_eq!(page(&st).form_error.as_deref(), Some(missing));
    assert_eq!(page(&st).open_blocked(de), Some(missing), "the Generate… button's tip");
    assert_eq!(page(&st).form_wait(de), Some(missing));

    let mut st = loaded(vec![], true);
    st.settings.language = "de".into();
    fill(&mut st, "keys/id", "", "", "");
    ssh(&mut st, SshMsg::Generate { overwrite: false });
    let relative = location_text(de, LocationProblem::Relative);
    assert_ne!(relative, location_text(Lang::En, LocationProblem::Relative));
    assert_eq!(page(&st).form_error.as_deref(), Some(relative));

    fill(
        &mut st,
        "~/.ssh/work",
        "a
b",
        "",
        "",
    );
    let control = page(&st).comment_problem(de).expect("a control character");
    assert_ne!(Some(control), page(&st).comment_problem(Lang::En));
    assert_eq!(page(&st).blocked(de), Some(control));

    page_mut(&mut st).comment.clear();
    page_mut(&mut st).generating = true;
    assert_eq!(page(&st).form_wait(de), Some(de.tr("prefs.ssh.generate.busy.warn")));
    for p in [LocationProblem::Empty, LocationProblem::PublicFile, LocationProblem::NotAFile, LocationProblem::WindowsName] {
        assert_ne!(location_text(de, p), location_text(Lang::En, p), "{p:?}");
    }
}

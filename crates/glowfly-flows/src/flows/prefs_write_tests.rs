//! Headless tests for when Preferences says "Saved": only once git reports the config write
//! succeeded. A failed write shows its error on the page, keeps the page edited (and counted by
//! the quit question), and shows no tick. The Commit signing page's text fields go through the
//! Save / Revert bar. (The Ctrl/Cmd+S and Esc keys are matched in `ui/shortcuts/prefs_key_tests.rs`.)

use super::prefs_save_tests::{edited, git_mut, identity, on_page, prefs, writes};
use crate::effect::{Effect, PrefsEffect};
use crate::msg::{FilePurpose, PrefsMsg};
use crate::state::prefs::GitConfigValues;
use crate::state::tab_losses::quit_lines;
use crate::state::{AppState, PrefPage};

/// The ticket of the one batched config write among `fx`.
fn ticket(fx: &[Effect]) -> u64 {
    let tickets: Vec<u64> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::Prefs(PrefsEffect::WriteGitConfig { ticket, .. }) => Some(*ticket),
            _ => None,
        })
        .collect();
    assert_eq!(tickets.len(), 1, "{fx:?}");
    tickets[0]
}

fn written(st: &mut AppState, ticket: u64, result: Result<(), String>) -> Vec<Effect> {
    prefs(st, PrefsMsg::ConfigSaved { ticket, result })
}

fn tick_pending(st: &AppState) -> bool {
    st.prefs.as_ref().unwrap().saved.pending
}

fn error(st: &AppState) -> Option<String> {
    crate::state::prefs_save::save_error(st.prefs.as_ref().unwrap()).map(str::to_owned)
}

#[test]
fn saved_waits_for_the_write_to_succeed() {
    let mut st = identity();
    git_mut(&mut st).repo_email = " ada@example.com ".into();
    let fx = prefs(&mut st, PrefsMsg::SavePage);
    assert_eq!(writes(&fx), [(false, "user.email", Some("ada@example.com"))]);
    assert!(!tick_pending(&st), "nothing is saved yet");
    assert!(edited(&st), "still edited while the write runs");
    let p = st.prefs.as_ref().unwrap();
    assert!(crate::state::prefs_save::saving(p), "the bar waits");
    // Saving again while it runs writes nothing more.
    assert!(writes(&prefs(&mut st, PrefsMsg::SavePage)).is_empty());

    let id = ticket(&fx);
    written(&mut st, id, Ok(()));
    assert!(tick_pending(&st), "shown on the next frame");
    assert!(!edited(&st), "the loaded copy now holds what was written");
    assert_eq!(git_mut(&mut st).repo_email, "ada@example.com", "the field is trimmed");
    assert!(error(&st).is_none());
    assert!(quit_lines(&st).is_empty(), "nothing left to lose");
}

#[test]
fn a_failed_write_shows_its_error_and_keeps_the_page_edited() {
    let mut st = identity();
    git_mut(&mut st).repo_name = "Grace".into();
    let fx = prefs(&mut st, PrefsMsg::SavePage);
    written(&mut st, ticket(&fx), Err("error: could not lock config file .git/config".into()));
    assert!(!tick_pending(&st), "no tick for a failed write");
    assert!(!st.prefs.as_ref().unwrap().saved.showing());
    assert_eq!(error(&st).as_deref(), Some("error: could not lock config file .git/config"));
    assert!(edited(&st), "the edits are still there to save again");
    assert_eq!(git_mut(&mut st).repo_name, "Grace");
    assert_eq!(quit_lines(&st), ["- Preferences: the changes on the Identity page that aren't saved yet"], "quitting still asks");
    assert!(st.log.iter().any(|l| !l.ok && l.text.contains("could not lock")), "in the activity log too");

    // Saving again clears the error; success shows the tick.
    let fx = prefs(&mut st, PrefsMsg::SavePage);
    assert_eq!(writes(&fx), [(false, "user.name", Some("Grace"))], "the failed key is written again");
    assert!(error(&st).is_none(), "a new try clears the old error");
    written(&mut st, ticket(&fx), Ok(()));
    assert!(tick_pending(&st) && !edited(&st));
}

#[test]
fn revert_clears_a_failed_save() {
    let mut st = identity();
    git_mut(&mut st).repo_name = "Grace".into();
    let fx = prefs(&mut st, PrefsMsg::SavePage);
    written(&mut st, ticket(&fx), Err("denied".into()));
    prefs(&mut st, PrefsMsg::RevertPage);
    assert!(error(&st).is_none());
    assert_eq!(git_mut(&mut st).repo_name, "Ada");
    assert!(!edited(&st));
}

#[test]
fn the_error_belongs_to_its_page() {
    let mut st = on_page(PrefPage::General, GitConfigValues::default());
    git_mut(&mut st).longpaths = true;
    let fx = prefs(&mut st, PrefsMsg::SavePage);
    st.prefs.as_mut().unwrap().page = PrefPage::Ui;
    written(&mut st, ticket(&fx), Err("denied".into()));
    assert!(error(&st).is_none(), "not shown on another page");
    st.prefs.as_mut().unwrap().page = PrefPage::General;
    assert_eq!(error(&st).as_deref(), Some("denied"), "back on its page");
    assert!(edited(&st));
}

#[test]
fn a_result_after_preferences_closed_is_logged() {
    let mut st = identity();
    git_mut(&mut st).repo_name = "Grace".into();
    let fx = prefs(&mut st, PrefsMsg::SavePage);
    st.prefs = None;
    written(&mut st, ticket(&fx), Err("denied".into()));
    assert!(st.log.iter().any(|l| !l.ok && l.text.contains("denied")));
}

#[test]
fn saving_an_unedited_page_does_nothing() {
    let mut st = identity();
    let fx = prefs(&mut st, PrefsMsg::SavePage);
    assert!(fx.is_empty(), "{fx:?}");
    assert!(!tick_pending(&st));
}

#[test]
fn a_signing_switch_shows_saved_once_written() {
    let mut st = on_page(PrefPage::Signing, GitConfigValues::default());
    let fx = prefs(&mut st, PrefsMsg::GitConfig { global: true, key: "commit.gpgsign".into(), value: Some("true".into()) });
    assert_eq!(writes(&fx), [(true, "commit.gpgsign", Some("true"))]);
    assert!(!tick_pending(&st));
    written(&mut st, ticket(&fx), Ok(()));
    assert!(tick_pending(&st));

    let fx = prefs(&mut st, PrefsMsg::GitConfig { global: true, key: "tag.gpgsign".into(), value: Some("true".into()) });
    written(&mut st, ticket(&fx), Err("denied".into()));
    assert_eq!(error(&st).as_deref(), Some("denied"));
}

#[test]
fn signing_program_and_allowed_signers_save_through_the_bar() {
    let mut st = on_page(PrefPage::Signing, GitConfigValues { signing_format: "ssh".into(), ..Default::default() });
    assert!(!edited(&st));
    git_mut(&mut st).ssh_program = " /usr/bin/ssh-keygen ".into();
    git_mut(&mut st).ssh_allowed_signers = "~/.ssh/allowed_signers".into();
    assert!(edited(&st), "the bar shows");
    let fx = prefs(&mut st, PrefsMsg::SavePage);
    assert_eq!(
        writes(&fx),
        [(true, "gpg.ssh.program", Some("/usr/bin/ssh-keygen")), (true, "gpg.ssh.allowedSignersFile", Some("~/.ssh/allowed_signers"))]
    );
    assert!(fx.iter().any(|e| matches!(e, Effect::Prefs(PrefsEffect::ListSigningKeys { .. }))), "keys listed with the new program");
    written(&mut st, ticket(&fx), Ok(()));
    assert!(!edited(&st) && tick_pending(&st));

    // Revert puts them back.
    git_mut(&mut st).ssh_allowed_signers.clear();
    assert!(edited(&st));
    prefs(&mut st, PrefsMsg::RevertPage);
    assert_eq!(git_mut(&mut st).ssh_allowed_signers, "~/.ssh/allowed_signers");
}

#[test]
fn browsing_for_a_signing_file_only_fills_the_field() {
    let mut st = on_page(PrefPage::Signing, GitConfigValues { signing_format: "ssh".into(), ..Default::default() });
    let path = glowfly_core::paths::temp_dir().join("allowed_signers");
    let fx = prefs(&mut st, PrefsMsg::FilePicked { purpose: FilePurpose::SshAllowedSigners, path: Some(path.clone()) });
    assert!(writes(&fx).is_empty(), "{fx:?}");
    assert_eq!(git_mut(&mut st).ssh_allowed_signers, path.display().to_string());
    assert!(edited(&st));
    let fx = prefs(&mut st, PrefsMsg::FilePicked { purpose: FilePurpose::SigningProgram, path: Some(path.clone()) });
    assert!(writes(&fx).is_empty());
    assert_eq!(git_mut(&mut st).ssh_program, path.display().to_string());
}

#[test]
fn leaving_a_signing_page_with_edits_asks_first() {
    let mut st = on_page(PrefPage::Signing, GitConfigValues::default());
    git_mut(&mut st).gpg_program = "gpg2".into();
    prefs(&mut st, PrefsMsg::Navigate(PrefPage::General));
    assert!(matches!(st.modal, Some(crate::state::Modal::ConfirmChoice { .. })));
    assert_eq!(st.prefs.as_ref().unwrap().page, PrefPage::Signing);
}

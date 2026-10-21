//! Headless tests for signing-passphrase prompts from the gpg.program wrapper: queueing,
//! answering, retries only when the wrapper says so, the preference that hands the prompt to
//! GnuPG, and what a cancel or timeout says about the job.

use super::tests::state;
use super::update;
use crate::effect::{Effect, UiEffect};
use crate::msg::{CredentialMsg, JobMsg, Msg};
use crate::state::{AppState, RunningJob};
use glowfly_core::askpass::{GpgAsk, Prompt, Reply};
use std::time::Instant;

const KEY: &str = "ABCDEF0123456789";

fn gpg(st: &mut AppState, id: u64, retry: bool) -> Vec<Effect> {
    let ask = GpgAsk { key: KEY.into(), user: Some("Ada Lovelace <ada@example.com>".into()), retry };
    send(st, Msg::Credentials(CredentialMsg::CredentialPrompt { id, prompt: Prompt::gpg(ask) }))
}

pub(super) fn send(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

pub(super) fn start_job(st: &mut AppState, label: &str) {
    st.job = Some(RunningJob {
        label: label.into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: false,
    });
}

pub(super) fn finish(st: &mut AppState, ok: bool) {
    send(st, Msg::Job(JobMsg::JobDone { ok, output: "$ git commit\nerror: gpg failed to sign the data".into(), undo: None }));
}

pub(super) fn last_toast(st: &AppState) -> Option<&str> {
    st.toasts.last().map(|t| t.text.as_str())
}

#[test]
fn a_signing_prompt_is_shown_and_answered() {
    let mut st = state();
    start_job(&mut st, "Commit");
    let fx = gpg(&mut st, 1, false);
    assert!(st.askpass.showing());
    assert!(fx.iter().any(|e| matches!(e, Effect::Ui(UiEffect::RequestAttention))));
    assert!(!st.askpass.can_submit(), "an empty passphrase can't be sent");

    st.askpass.secret = "right".into();
    assert!(st.askpass.can_submit());
    let fx = send(&mut st, Msg::Credentials(CredentialMsg::AnswerCredentials));
    let answered: Vec<_> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::AnswerPrompt { id, reply: Reply::Answer(s) } => Some((*id, s.expose().to_owned())),
            _ => None,
        })
        .collect();
    assert_eq!(answered, [(1, "right".to_owned())]);
    assert!(st.askpass.secret.is_empty() && st.askpass.user.is_empty());
    assert!(st.askpass.answered_hosts.is_empty(), "a signing key is not a host");
    assert!(!st.askpass.showing());
}

#[test]
fn a_retry_comes_only_from_the_wrapper() {
    let mut st = state();
    start_job(&mut st, "Commit");
    gpg(&mut st, 1, false);
    st.askpass.secret = "wrong".into();
    send(&mut st, Msg::Credentials(CredentialMsg::AnswerCredentials));
    // The same prompt text again, but the wrapper doesn't call it a retry.
    gpg(&mut st, 2, false);
    assert!(!st.askpass.front().unwrap().retry);
    st.askpass.secret = "x".into();
    send(&mut st, Msg::Credentials(CredentialMsg::AnswerCredentials));
    gpg(&mut st, 3, true);
    assert!(st.askpass.front().unwrap().retry);
    assert!(!st.askpass.front().unwrap().rejected_before);
}

#[test]
fn gnupgs_own_window_when_the_preference_is_off() {
    let mut st = state();
    st.settings.gpg_prompts_in_app = false;
    let fx = gpg(&mut st, 4, false);
    assert!(matches!(fx.as_slice(), [Effect::AnswerPrompt { id: 4, reply: Reply::UseSystem }]), "{fx:?}");
    assert!(!st.askpass.showing());
}

#[test]
fn a_cancelled_signing_prompt_explains_the_failed_job() {
    let mut st = state();
    start_job(&mut st, "Commit");
    gpg(&mut st, 1, false);
    let fx = send(&mut st, Msg::Credentials(CredentialMsg::CancelCredentials));
    assert!(matches!(fx.as_slice(), [Effect::AnswerPrompt { id: 1, reply: Reply::Cancel }]), "{fx:?}");
    assert!(st.askpass.ended_signing);
    finish(&mut st, false);
    assert_eq!(last_toast(&st), Some("Commit cancelled: no signing passphrase was given"));
    assert!(!st.askpass.ended_signing, "reset with the job");

    // The next job's credential prompt reads as before.
    start_job(&mut st, "Push");
    send(&mut st, Msg::Credentials(CredentialMsg::CredentialPrompt { id: 2, prompt: Prompt::new("Username for 'https://x.org': ", None) }));
    send(&mut st, Msg::Credentials(CredentialMsg::CancelCredentials));
    finish(&mut st, false);
    assert_eq!(last_toast(&st), Some("Push cancelled: no credentials were given"));
}

#[test]
fn an_expired_signing_prompt_explains_the_failed_job() {
    let mut st = state();
    start_job(&mut st, "Commit");
    gpg(&mut st, 1, false);
    send(&mut st, Msg::Credentials(CredentialMsg::CredentialPromptExpired(1)));
    assert!(!st.askpass.showing());
    assert_eq!(last_toast(&st), Some("The signing passphrase prompt timed out."));
    finish(&mut st, false);
    assert_eq!(last_toast(&st), Some("Commit failed: the signing passphrase prompt timed out"));
    assert!(!st.askpass.ended_signing);
}

#[test]
fn a_failed_signing_job_flags_no_host() {
    let mut st = state();
    st.askpass.failed_hosts.insert("github.com".into());
    start_job(&mut st, "Commit");
    gpg(&mut st, 1, false);
    st.askpass.secret = "wrong".into();
    send(&mut st, Msg::Credentials(CredentialMsg::AnswerCredentials));
    finish(&mut st, false);
    assert_eq!(st.askpass.failed_hosts.len(), 1);
    assert!(st.askpass.failed_hosts.contains("github.com"));
}

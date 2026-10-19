//! Headless tests for credential prompts: queueing, answering (with the kept password for git's
//! follow-up prompt), cancelling, timeouts, and what a finished job clears.

use super::tests::state;
use super::update;
use crate::effect::{Effect, UiEffect};
use crate::msg::CredentialMsg;
use crate::msg::JobMsg;
use crate::msg::Msg;
use crate::state::askpass::{Carry, PromptEnd};
use crate::state::{AppState, RunningJob};
use std::time::Instant;
use trellis_core::askpass::{Prompt, PromptKind, Reply, Secret};

const USERNAME: &str = "Username for 'https://github.com': ";
const PASSWORD: &str = "Password for 'https://ada@github.com': ";
const PASSPHRASE: &str = "Enter passphrase for key '/home/ada/.ssh/id_ed25519': ";

fn ask(st: &mut AppState, id: u64, text: &str) -> Vec<Effect> {
    update(st, Msg::Credentials(CredentialMsg::CredentialPrompt { id, prompt: Prompt::new(text, None) }), Instant::now())
}

fn send(st: &mut AppState, msg: Msg) -> Vec<Effect> {
    update(st, msg, Instant::now())
}

/// The prompt id and answer text of an `AnswerPrompt` (None for a cancel).
fn answers(fx: &[Effect]) -> Vec<(u64, Option<String>)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::AnswerPrompt { id, reply: Reply::Answer(s) } => Some((*id, Some(s.expose().to_owned()))),
            Effect::AnswerPrompt { id, reply: Reply::Cancel } => Some((*id, None)),
            _ => None,
        })
        .collect()
}

fn start_job(st: &mut AppState, label: &str) {
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

fn finish(st: &mut AppState, ok: bool) -> Vec<Effect> {
    send(st, Msg::Job(JobMsg::JobDone { ok, output: "$ git push\nfatal: Authentication failed".into(), undo: None }))
}

#[test]
fn sign_in_answers_the_username_then_the_password_prompt() {
    let mut st = state();
    let fx = ask(&mut st, 1, USERNAME);
    assert!(st.askpass.showing());
    assert!(fx.iter().any(|e| matches!(e, Effect::Ui(UiEffect::RequestAttention))));
    assert!(st.askpass.focus_pending);

    st.askpass.user = "ada".into();
    st.askpass.secret = "hunter2".into();
    let fx = send(&mut st, Msg::Credentials(CredentialMsg::AnswerCredentials));
    assert_eq!(answers(&fx), [(1, Some("ada".into()))]);
    assert!(!st.askpass.showing());
    assert!(st.askpass.user.is_empty() && st.askpass.secret.is_empty());
    assert!(st.askpass.carry.is_some());

    // git's follow-up prompt is answered at once with the kept password.
    let fx = ask(&mut st, 2, PASSWORD);
    assert_eq!(answers(&fx), [(2, Some("hunter2".into()))]);
    assert!(!st.askpass.showing());
    assert!(st.askpass.carry.is_none(), "the password is dropped once used");
}

#[test]
fn the_kept_password_only_answers_its_own_host_and_user() {
    for other in ["Password for 'https://ada@gitlab.com': ", "Password for 'https://bob@github.com': "] {
        let mut st = state();
        ask(&mut st, 1, USERNAME);
        st.askpass.user = "ada".into();
        st.askpass.secret = "hunter2".into();
        send(&mut st, Msg::Credentials(CredentialMsg::AnswerCredentials));
        let fx = ask(&mut st, 2, other);
        assert!(answers(&fx).is_empty(), "{other}");
        assert_eq!(st.askpass.front().map(|p| p.id), Some(2));
    }
}

#[test]
fn answers_by_kind() {
    let mut st = state();
    ask(&mut st, 1, USERNAME);
    assert!(answers(&send(&mut st, Msg::Credentials(CredentialMsg::AnswerCredentials))).is_empty(), "an empty username can't be sent");
    assert!(st.askpass.showing());
    send(&mut st, Msg::Credentials(CredentialMsg::CancelCredentials));

    ask(&mut st, 2, "Are you sure you want to continue connecting (yes/no/[fingerprint])? ");
    assert_eq!(answers(&send(&mut st, Msg::Credentials(CredentialMsg::AnswerCredentials))), [(2, Some("yes".into()))]);

    ask(&mut st, 3, "Verification code: ");
    st.askpass.user = "123456".into();
    assert_eq!(answers(&send(&mut st, Msg::Credentials(CredentialMsg::AnswerCredentials))), [(3, Some("123456".into()))]);

    ask(&mut st, 4, PASSPHRASE);
    assert!(answers(&send(&mut st, Msg::Credentials(CredentialMsg::AnswerCredentials))).is_empty(), "an empty passphrase can't be sent");
    st.askpass.secret = "open sesame".into();
    assert_eq!(answers(&send(&mut st, Msg::Credentials(CredentialMsg::AnswerCredentials))), [(4, Some("open sesame".into()))]);
}

#[test]
fn a_cancelled_prompt_explains_the_failed_job() {
    let mut st = state();
    start_job(&mut st, "Push");
    ask(&mut st, 1, USERNAME);
    let fx = send(&mut st, Msg::Credentials(CredentialMsg::CancelCredentials));
    assert_eq!(answers(&fx), [(1, None)]);
    assert_eq!(st.askpass.ended, Some(PromptEnd::Cancelled));
    finish(&mut st, false);
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Push cancelled: no credentials were given"));
    assert_eq!(st.askpass.ended, None);
}

#[test]
fn an_expired_prompt_closes_and_explains_the_failed_job() {
    let mut st = state();
    start_job(&mut st, "Push");
    ask(&mut st, 1, USERNAME);
    send(&mut st, Msg::Credentials(CredentialMsg::CredentialPromptExpired(1)));
    assert!(!st.askpass.showing());
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("The credential prompt timed out."));
    finish(&mut st, false);
    assert_eq!(st.toasts.last().map(|t| t.text.as_str()), Some("Push failed: the credential prompt timed out"));
}

#[test]
fn a_prompt_asked_again_in_the_same_job_is_a_retry() {
    let mut st = state();
    start_job(&mut st, "Fetch");
    ask(&mut st, 1, PASSPHRASE);
    assert!(!st.askpass.front().unwrap().retry);
    st.askpass.secret = "wrong".into();
    send(&mut st, Msg::Credentials(CredentialMsg::AnswerCredentials));
    ask(&mut st, 2, PASSPHRASE);
    assert!(st.askpass.front().unwrap().retry);
    st.askpass.secret = "right".into();
    send(&mut st, Msg::Credentials(CredentialMsg::AnswerCredentials));
    finish(&mut st, true);

    start_job(&mut st, "Fetch");
    ask(&mut st, 3, PASSPHRASE);
    assert!(!st.askpass.front().unwrap().retry, "a new job starts afresh");
}

#[test]
fn credentials_that_failed_are_flagged_next_time() {
    let mut st = state();
    start_job(&mut st, "Push");
    ask(&mut st, 1, USERNAME);
    st.askpass.user = "ada".into();
    send(&mut st, Msg::Credentials(CredentialMsg::AnswerCredentials));
    finish(&mut st, false);
    assert_ne!(st.toasts.last().map(|t| t.text.as_str()), Some("Push cancelled: no credentials were given"));

    start_job(&mut st, "Push");
    ask(&mut st, 2, USERNAME);
    assert!(st.askpass.front().unwrap().rejected_before);
    st.askpass.user = "ada".into();
    send(&mut st, Msg::Credentials(CredentialMsg::AnswerCredentials));
    finish(&mut st, true);

    start_job(&mut st, "Push");
    ask(&mut st, 3, USERNAME);
    assert!(!st.askpass.front().unwrap().rejected_before, "a successful job clears it");
}

#[test]
fn a_finished_job_cancels_what_it_left() {
    let mut st = state();
    start_job(&mut st, "Push");
    ask(&mut st, 1, USERNAME);
    ask(&mut st, 2, PASSPHRASE);
    st.askpass.carry = Some(Carry { host: "github.com".into(), user: "ada".into(), password: Secret::new("hunter2") });
    let fx = finish(&mut st, false);
    assert_eq!(answers(&fx), [(1, None), (2, None)]);
    assert!(!st.askpass.showing());
    assert!(st.askpass.carry.is_none());

    let fx = [Effect::AnswerPrompt { id: 7, reply: Reply::Answer(Secret::new("hunter2")) }];
    assert!(!format!("{fx:?}").contains("hunter2"));
}

#[test]
fn carry_and_submit_rules() {
    let mut st = state();
    let p = &mut st.askpass;
    p.carry = Some(Carry { host: "dev.azure.com".into(), user: "ada@corp.com".into(), password: Secret::new("x") });
    assert!(p.carry_answers(&Prompt::new("Password for 'https://ada%40corp.com@dev.azure.com': ", None)));
    assert!(!p.carry_answers(&Prompt::new("Password for 'https://ada%40corp.com@github.com': ", None)));
    assert!(!p.carry_answers(&Prompt::new(PASSPHRASE, None)));
    p.carry = Some(Carry { host: "github.com".into(), user: "ada".into(), password: Secret::new("x") });
    assert!(p.carry_answers(&Prompt::new(PASSWORD, None)));

    let mut st = state();
    for (text, empty_ok) in [(USERNAME, false), (PASSWORD, false), (PASSPHRASE, false), ("Continue (yes/no)? ", true)] {
        ask(&mut st, 1, text);
        assert_eq!(st.askpass.can_submit(), empty_ok, "{text}");
        st.askpass.user = "ada".into();
        st.askpass.secret = "s".into();
        assert!(st.askpass.can_submit(), "{text}");
        send(&mut st, Msg::Credentials(CredentialMsg::CancelCredentials));
    }
    assert!(matches!(Prompt::new("Enter PIN: ", None).kind, PromptKind::Other { secret: true }));
}

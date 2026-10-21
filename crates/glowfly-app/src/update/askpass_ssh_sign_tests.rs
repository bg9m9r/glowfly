//! Headless tests for ssh-keygen's signing-passphrase prompts (git signing with an SSH key):
//! always shown in Trellis, never a retry, and what a cancel, a timeout or a wrong passphrase
//! says about the job.

use super::askpass_gpg_tests::{finish, last_toast, send, start_job};
use super::tests::state;
use crate::effect::{Effect, UiEffect};
use crate::msg::{CredentialMsg, JobMsg, Msg};
use crate::state::AppState;
use glowfly_core::askpass::{Prompt, Reply, SshSignAsk};

const WRONG: &str = "$ git commit\nLoad key \"/home/ada/.ssh/id_ed25519\": incorrect passphrase supplied to decrypt private key\n\
fatal: failed to write commit object";

fn ssh(st: &mut AppState, id: u64) -> Vec<Effect> {
    let prompt = Prompt::ssh_sign(SshSignAsk { key: "~/.ssh/id_ed25519".into() });
    send(st, Msg::Credentials(CredentialMsg::CredentialPrompt { id, prompt }))
}

fn answer(st: &mut AppState, secret: &str) -> Vec<Effect> {
    st.askpass.secret = secret.into();
    send(st, Msg::Credentials(CredentialMsg::AnswerCredentials))
}

fn done(st: &mut AppState, ok: bool, output: &str) {
    send(st, Msg::Job(JobMsg::JobDone { ok, output: output.into(), undo: None }));
}

#[test]
fn an_ssh_signing_prompt_is_shown_even_when_gnupgs_window_is_preferred() {
    let mut st = state();
    st.settings.gpg_prompts_in_app = false;
    start_job(&mut st, "Commit");
    let fx = ssh(&mut st, 1);
    assert!(st.askpass.showing());
    assert!(fx.iter().any(|e| matches!(e, Effect::Ui(UiEffect::RequestAttention))));
    assert!(!fx.iter().any(|e| matches!(e, Effect::AnswerPrompt { reply: Reply::UseSystem, .. })), "{fx:?}");
}

#[test]
fn an_ssh_signing_prompt_is_never_a_retry() {
    let mut st = state();
    start_job(&mut st, "Rebase");
    ssh(&mut st, 1);
    assert!(!st.askpass.front().unwrap().retry);
    assert!(!st.askpass.can_submit(), "an empty passphrase can't be sent");
    let fx = answer(&mut st, "x");
    let answered: Vec<_> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::AnswerPrompt { id, reply: Reply::Answer(s) } => Some((*id, s.expose().to_owned())),
            _ => None,
        })
        .collect();
    assert_eq!(answered, [(1, "x".to_owned())]);
    assert!(st.askpass.secret.is_empty() && st.askpass.user.is_empty());
    assert!(st.askpass.answered_hosts.is_empty(), "a signing key is not a host");
    // The next commit of the rebase asks again: the next signature, not a second try.
    ssh(&mut st, 2);
    assert!(!st.askpass.front().unwrap().retry);
}

#[test]
fn a_cancelled_ssh_signing_prompt_explains_the_failed_job() {
    let mut st = state();
    start_job(&mut st, "Commit");
    ssh(&mut st, 1);
    let fx = send(&mut st, Msg::Credentials(CredentialMsg::CancelCredentials));
    assert!(matches!(fx.as_slice(), [Effect::AnswerPrompt { id: 1, reply: Reply::Cancel }]), "{fx:?}");
    assert!(st.askpass.ended_signing);
    finish(&mut st, false);
    assert_eq!(last_toast(&st), Some("Commit cancelled: no signing passphrase was given"));
}

#[test]
fn an_expired_ssh_signing_prompt_explains_the_failed_job() {
    let mut st = state();
    start_job(&mut st, "Commit");
    ssh(&mut st, 1);
    send(&mut st, Msg::Credentials(CredentialMsg::CredentialPromptExpired(1)));
    assert!(!st.askpass.showing());
    assert_eq!(last_toast(&st), Some("The signing passphrase prompt timed out."));
    finish(&mut st, false);
    assert_eq!(last_toast(&st), Some("Commit failed: the signing passphrase prompt timed out"));
}

#[test]
fn a_wrong_ssh_passphrase_gets_its_own_headline() {
    let mut st = state();
    start_job(&mut st, "Commit");
    ssh(&mut st, 1);
    answer(&mut st, "wrong");
    assert!(st.askpass.answered_ssh_signing);
    done(&mut st, false, WRONG);
    assert_eq!(last_toast(&st), Some("Commit failed: the passphrase didn't unlock the SSH signing key"));
    assert!(!st.askpass.answered_ssh_signing, "reset with the job");

    // The same failure without a passphrase from Trellis keeps git's own text.
    st.toasts.clear();
    start_job(&mut st, "Commit");
    done(&mut st, false, WRONG);
    assert!(st.toasts.iter().all(|t| !t.text.contains("didn't unlock the SSH signing key")));
    assert_ne!(last_toast(&st), Some("Commit failed: the passphrase didn't unlock the SSH signing key"));
}

#[test]
fn an_ssh_connection_passphrase_is_not_a_signing_prompt() {
    let mut st = state();
    start_job(&mut st, "Push");
    let prompt = Prompt::new("Enter passphrase for key '~/.ssh/id_rsa': ", None);
    send(&mut st, Msg::Credentials(CredentialMsg::CredentialPrompt { id: 1, prompt }));
    send(&mut st, Msg::Credentials(CredentialMsg::CancelCredentials));
    assert!(!st.askpass.ended_signing);
    finish(&mut st, false);
    assert_eq!(last_toast(&st), Some("Push cancelled: no credentials were given"));
}

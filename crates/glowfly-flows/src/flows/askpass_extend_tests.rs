//! Preferences' own passphrase prompt (Extend by 2 years) beside a git job's: cancelling it or
//! letting it time out says nothing about the job, and a job that ends leaves it open.

use super::askpass_gpg_tests::{finish, last_toast, send, start_job};
use super::tests::state;
use crate::effect::Effect;
use crate::msg::{CredentialMsg, Msg};
use crate::state::AppState;
use glowfly_core::askpass::{GpgAsk, Prompt, Reply};

fn extend_prompt(st: &mut AppState, id: u64) {
    let ask = GpgAsk { key: "0123456789ABCDEF0123456789ABCDEF01234567".into(), user: None, retry: false, extend: true };
    send(st, Msg::Credentials(CredentialMsg::CredentialPrompt { id, prompt: Prompt::gpg(ask) }));
}

/// The toast a failed Commit job shows with no prompt involved.
fn plain_failure() -> String {
    let mut st = state();
    start_job(&mut st, "Commit");
    finish(&mut st, false);
    last_toast(&st).expect("a failure toast").to_owned()
}

#[test]
fn a_cancelled_extend_prompt_leaves_the_next_job_its_own_headline() {
    let mut st = state();
    extend_prompt(&mut st, 1);
    let fx = send(&mut st, Msg::Credentials(CredentialMsg::CancelCredentials));
    assert!(matches!(fx.as_slice(), [Effect::AnswerPrompt { id: 1, reply: Reply::Cancel }]));
    assert_eq!(st.askpass.ended, None);
    assert!(!st.askpass.ended_signing);
    start_job(&mut st, "Commit");
    finish(&mut st, false);
    assert_eq!(last_toast(&st).map(str::to_owned), Some(plain_failure()));
}

#[test]
fn a_timed_out_extend_prompt_leaves_the_next_job_its_own_headline() {
    let mut st = state();
    extend_prompt(&mut st, 1);
    send(&mut st, Msg::Credentials(CredentialMsg::CredentialPromptExpired(1)));
    assert!(!st.askpass.showing());
    assert_eq!(st.askpass.ended, None, "Preferences reports its own timeout");
    start_job(&mut st, "Commit");
    finish(&mut st, false);
    assert_eq!(last_toast(&st).map(str::to_owned), Some(plain_failure()));
}

#[test]
fn a_job_that_ends_leaves_the_extend_prompt_open() {
    let mut st = state();
    start_job(&mut st, "Fetch");
    extend_prompt(&mut st, 7);
    send(&mut st, Msg::Credentials(CredentialMsg::CredentialPrompt { id: 8, prompt: Prompt::new("Username for 'https://x.org': ", None) }));
    st.askpass.secret = "typed".into();
    let fx = send(&mut st, Msg::Job(crate::msg::JobMsg::JobDone { ok: true, output: String::new(), undo: None }));
    let cancelled: Vec<u64> = fx
        .iter()
        .filter_map(|e| match e {
            Effect::AnswerPrompt { id, reply: Reply::Cancel } => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(cancelled, [8], "only the job's own prompt is cancelled");
    assert!(st.askpass.showing());
    assert_eq!(st.askpass.queue.front().map(|q| q.id), Some(7));
    assert_eq!(st.askpass.secret, "typed", "what is typed into the open prompt stays");
}

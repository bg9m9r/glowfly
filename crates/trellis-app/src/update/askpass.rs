//! Credential prompts: queueing what git asks, answering from the dialog, and tidying up when
//! the job that asked ends.

use crate::effect::{Effect, UiEffect};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::askpass::{Carry, PendingPrompt, PromptEnd};
use std::time::Instant;
use trellis_core::askpass::{self, Prompt, PromptKind, Reply, Secret};

pub(super) fn handle(st: &mut AppState, msg: Msg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        Msg::CredentialPrompt { id, prompt } => asked(st, id, prompt, fx),
        Msg::AnswerCredentials => answer(st, fx),
        Msg::CancelCredentials => {
            let p = &mut st.askpass;
            let Some(front) = p.queue.pop_front() else { return };
            fx.push(Effect::AnswerPrompt { id: front.id, reply: Reply::Cancel });
            p.ended = Some(PromptEnd::Cancelled);
            p.carry = None;
            p.clear_buffers();
        }
        Msg::CredentialPromptExpired(id) => {
            let p = &mut st.askpass;
            let Some(i) = p.queue.iter().position(|q| q.id == id) else { return };
            p.queue.remove(i);
            if i == 0 {
                p.clear_buffers();
            }
            p.ended = Some(PromptEnd::TimedOut);
            st.toast(false, "The credential prompt timed out.", now);
            st.log(false, "The credential prompt timed out.");
        }
        _ => {}
    }
}

fn asked(st: &mut AppState, id: u64, prompt: Prompt, fx: &mut Vec<Effect>) {
    let p = &mut st.askpass;
    if p.carry_answers(&prompt)
        && let Some(carry) = p.carry.take()
    {
        p.answered.push(prompt.text);
        fx.push(Effect::AnswerPrompt { id, reply: Reply::Answer(carry.password) });
        return;
    }
    let retry = p.answered.contains(&prompt.text);
    let rejected_before = matches!(prompt.kind, PromptKind::Username { .. } | PromptKind::Password { .. })
        && askpass::host(&prompt.kind).is_some_and(|h| p.failed_hosts.contains(&h));
    let first = p.queue.is_empty();
    p.queue.push_back(PendingPrompt { id, prompt, retry, rejected_before });
    if first {
        p.clear_buffers();
        fx.push(Effect::Ui(UiEffect::RequestAttention));
    }
}

fn answer(st: &mut AppState, fx: &mut Vec<Effect>) {
    let p = &mut st.askpass;
    if !p.can_submit() {
        return;
    }
    let Some(front) = p.queue.pop_front() else { return };
    let host = askpass::host(&front.prompt.kind);
    let answer = match &front.prompt.kind {
        PromptKind::Username { .. } => {
            let user = p.user.trim().to_owned();
            if !p.secret.is_empty() {
                let password = Secret::new(std::mem::take(&mut p.secret));
                p.carry = Some(Carry { host: host.clone().unwrap_or_default(), user: user.clone(), password });
            }
            Secret::new(user)
        }
        PromptKind::Confirm => Secret::new("yes"),
        PromptKind::Other { secret: false } => Secret::new(std::mem::take(&mut p.user)),
        _ => Secret::new(std::mem::take(&mut p.secret)),
    };
    p.answered.push(front.prompt.text);
    p.answered_hosts.extend(host);
    p.clear_buffers();
    fx.push(Effect::AnswerPrompt { id: front.id, reply: Reply::Answer(answer) });
}

/// The running job ended: returns the headline for a job that failed because a prompt was
/// cancelled or timed out, remembers which hosts' credentials were not accepted, and cancels
/// and forgets whatever the job left behind.
pub(super) fn job_finished(st: &mut AppState, ok: bool, label: &str, fx: &mut Vec<Effect>) -> Option<String> {
    let p = &mut st.askpass;
    let headline = match (ok, p.ended) {
        (false, Some(PromptEnd::Cancelled)) => Some(format!("{label} cancelled: no credentials were given")),
        (false, Some(PromptEnd::TimedOut)) => Some(format!("{label} failed: the credential prompt timed out")),
        _ => None,
    };
    if ok {
        p.failed_hosts.retain(|h| !p.answered_hosts.contains(h));
    } else if p.ended.is_none() {
        p.failed_hosts.extend(p.answered_hosts.iter().cloned());
    }
    let left = std::mem::take(&mut p.queue);
    if !left.is_empty() {
        p.clear_buffers();
    }
    fx.extend(left.into_iter().map(|q| Effect::AnswerPrompt { id: q.id, reply: Reply::Cancel }));
    p.carry = None;
    p.answered.clear();
    p.answered_hosts.clear();
    p.ended = None;
    headline
}

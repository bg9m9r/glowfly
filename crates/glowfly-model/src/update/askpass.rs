//! Credential prompts: queueing what git asks, answering from the dialog, and tidying up when
//! the job that asked ends. Saving typed HTTPS sign-ins is in update/askpass_store.rs, the
//! sign-in banner for auto-fetch in update/sign_in.rs.

use crate::effect::{Effect, UiEffect};
use crate::msg::CredentialMsg;
use crate::state::AppState;
use crate::state::askpass::{Carry, PendingPrompt, PromptEnd};
use glowfly_core::askpass::{self, Prompt, PromptKind, Reply, Secret};
use glowfly_core::git_credential::HttpsCredential;
use std::path::Path;
use std::time::Instant;

pub(super) fn handle(st: &mut AppState, msg: CredentialMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        CredentialMsg::CredentialPrompt { id, prompt } => asked(st, id, prompt, fx),
        CredentialMsg::AnswerCredentials => answer(st, fx),
        CredentialMsg::CancelCredentials => {
            let p = &mut st.askpass;
            let Some(front) = p.queue.pop_front() else { return };
            fx.push(Effect::AnswerPrompt { id: front.id, reply: Reply::Cancel });
            p.clear_buffers();
            // Preferences' own prompt says nothing about the running job.
            if front.prompt.kind.is_app_own() {
                return;
            }
            p.ended = Some(PromptEnd::Cancelled);
            p.ended_signing = front.prompt.kind.is_signing();
            p.carry = None;
        }
        CredentialMsg::CredentialPromptExpired(id) => {
            let p = &mut st.askpass;
            let Some(i) = p.queue.iter().position(|q| q.id == id) else { return };
            let Some(gone) = p.queue.remove(i) else { return };
            if i == 0 {
                p.clear_buffers();
            }
            // Preferences reports its own prompt's timeout (`PrefsMsg::KeyExtended`).
            if gone.prompt.kind.is_app_own() {
                return;
            }
            let signing = gone.prompt.kind.is_signing();
            p.ended = Some(PromptEnd::TimedOut);
            p.ended_signing = signing;
            let text = if signing { "The signing passphrase prompt timed out." } else { "The credential prompt timed out." };
            st.toast(false, text, now);
            st.log(false, text);
        }
        CredentialMsg::Remembered { host, result } => super::askpass_store::remembered(st, &host, result, now),
        CredentialMsg::Forgotten { host, result } => super::askpass_store::forgotten(st, &host, result),
        CredentialMsg::SignInFetch { workdir, remote } => super::sign_in::fetch(st, &workdir, &remote, fx),
        CredentialMsg::DismissSignIn { workdir, remote } => st.sign_in.dismiss(&workdir, &remote),
    }
}

fn asked(st: &mut AppState, id: u64, prompt: Prompt, fx: &mut Vec<Effect>) {
    // An OpenPGP signing passphrase: GnuPG's own window when the user prefers it, and a retry
    // only when the wrapper says the last passphrase was wrong.
    let signing_retry = match &prompt.kind {
        PromptKind::GpgPassphrase { .. } if !st.settings.gpg_prompts_in_app => {
            fx.push(Effect::AnswerPrompt { id, reply: Reply::UseSystem });
            return;
        }
        PromptKind::GpgPassphrase { retry, .. } => Some(*retry),
        // ssh-keygen gives up after one wrong passphrase, so a second prompt in the same job is
        // the next signature (a rebase signs every commit), never a second try.
        PromptKind::SshSigningPassphrase { .. } => Some(false),
        _ => None,
    };
    let remember = st.settings.remember_https_credentials;
    let p = &mut st.askpass;
    // The kept password was recorded with the username it came with.
    if p.carry_answers(&prompt)
        && let Some(carry) = p.carry.take()
    {
        p.answered.push(prompt.text);
        fx.push(Effect::AnswerPrompt { id, reply: Reply::Answer(carry.password) });
        return;
    }
    let retry = signing_retry.unwrap_or_else(|| p.answered.contains(&prompt.text));
    let rejected_before = matches!(prompt.kind, PromptKind::Username { .. } | PromptKind::Password { .. })
        && askpass::host(&prompt.kind).is_some_and(|h| p.failed_hosts.contains(&h));
    let first = p.queue.is_empty();
    p.queue.push_back(PendingPrompt { id, prompt, retry, rejected_before });
    if first {
        p.clear_buffers();
        p.remember = remember;
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
    if matches!(front.prompt.kind, PromptKind::SshSigningPassphrase { .. }) {
        p.answered_ssh_signing = true;
    }
    let answer = match &front.prompt.kind {
        PromptKind::Username { url } => {
            let user = p.user.trim().to_owned();
            if !p.secret.is_empty() {
                let password = Secret::new(std::mem::take(&mut p.secret));
                if p.remember {
                    p.typed.extend(HttpsCredential::from_prompt(url, &user, password.clone()));
                }
                p.carry = Some(Carry { host: host.clone().unwrap_or_default(), user: user.clone(), password });
            }
            Secret::new(user)
        }
        PromptKind::Confirm | PromptKind::HostKey(_) => Secret::new("yes"),
        PromptKind::Other { secret: false } => Secret::new(std::mem::take(&mut p.user)),
        PromptKind::Password { target, user: Some(u) } if p.remember => {
            let secret = Secret::new(std::mem::take(&mut p.secret));
            p.typed.extend(HttpsCredential::from_prompt(target, u, secret.clone()));
            secret
        }
        _ => Secret::new(std::mem::take(&mut p.secret)),
    };
    if !front.prompt.kind.is_app_own() {
        p.answered.push(front.prompt.text);
        p.answered_hosts.extend(host);
    }
    p.clear_buffers();
    fx.push(Effect::AnswerPrompt { id: front.id, reply: Reply::Answer(answer) });
}

/// The running job ended: returns the headline for a job that failed because a prompt was
/// cancelled or timed out, remembers which hosts' credentials were not accepted, saves or drops
/// the HTTPS sign-ins typed with Remember on (run in `cwd`, judged by the job's `output`), and
/// cancels and forgets whatever the job left behind. A `stopped` job (cancelled from the status
/// bar) keeps and forgets no sign-in.
pub(super) fn job_finished(
    st: &mut AppState,
    ok: bool,
    stopped: bool,
    label: &str,
    cwd: Option<&Path>,
    output: &str,
    fx: &mut Vec<Effect>,
) -> Option<String> {
    let p = &mut st.askpass;
    let headline = match (ok, p.ended, p.ended_signing) {
        (false, Some(PromptEnd::Cancelled), false) => Some(format!("{label} cancelled: no credentials were given")),
        (false, Some(PromptEnd::TimedOut), false) => Some(format!("{label} failed: the credential prompt timed out")),
        (false, Some(PromptEnd::Cancelled), true) => Some(format!("{label} cancelled: no signing passphrase was given")),
        (false, Some(PromptEnd::TimedOut), true) => Some(format!("{label} failed: the signing passphrase prompt timed out")),
        _ => None,
    }
    .or_else(|| {
        let wrong = !ok && p.ended.is_none() && p.answered_ssh_signing && askpass::ssh_sign::wrong_passphrase(output);
        wrong.then(|| format!("{label} failed: the passphrase didn't unlock the SSH signing key"))
    });
    // A stopped job says nothing about the sign-ins it was given: none is kept or forgotten.
    if ok {
        p.failed_hosts.retain(|h| !p.answered_hosts.contains(h));
    } else if p.ended.is_none() && !stopped {
        p.failed_hosts.extend(p.answered_hosts.iter().cloned());
    }
    if stopped {
        p.typed.clear();
    }
    super::askpass_store::settle(p, ok, cwd, output, fx);
    // Preferences' own prompt outlives the job; the job's are cancelled.
    let front_was_app_own = p.queue.front().is_some_and(|q| q.prompt.kind.is_app_own());
    let mut left = Vec::new();
    for q in std::mem::take(&mut p.queue) {
        if q.prompt.kind.is_app_own() { p.queue.push_back(q) } else { left.push(q) }
    }
    if !left.is_empty() && !front_was_app_own {
        p.clear_buffers();
    }
    fx.extend(left.into_iter().map(|q| Effect::AnswerPrompt { id: q.id, reply: Reply::Cancel }));
    p.carry = None;
    p.answered.clear();
    p.answered_hosts.clear();
    p.ended = None;
    p.ended_signing = false;
    p.answered_ssh_signing = false;
    headline
}

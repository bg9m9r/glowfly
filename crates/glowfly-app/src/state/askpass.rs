//! Credential prompts from git and ssh (through the askpass helper) waiting for the user.
//! Global, not parked with a tab: a prompt belongs to the running job, whichever tab is shown.

use glowfly_core::askpass::{self, Prompt, PromptKind, Secret};
use glowfly_core::git_credential::HttpsCredential;
use std::collections::{HashSet, VecDeque};

/// A prompt waiting in the queue.
#[derive(Clone, Debug)]
pub struct PendingPrompt {
    pub id: u64,
    pub prompt: Prompt,
    /// The answer to the same prompt earlier in this job didn't work (for a signing passphrase:
    /// the wrapper said so).
    pub retry: bool,
    /// The last job that used credentials Glowfly supplied for this host failed.
    pub rejected_before: bool,
}

/// How the current job's prompting ended early.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PromptEnd {
    Cancelled,
    TimedOut,
}

/// The password typed with a username, kept for git's follow-up password prompt.
pub struct Carry {
    pub host: String,
    pub user: String,
    pub password: Secret,
}

#[derive(Default)]
pub struct CredentialPrompts {
    /// Oldest first; the front one is shown.
    pub queue: VecDeque<PendingPrompt>,
    /// The dialog's text buffers (edited by the view).
    pub user: String,
    pub secret: String,
    /// View state: show the masked field's text; focus the first field on the next frame.
    pub reveal: bool,
    pub focus_pending: bool,
    pub carry: Option<Carry>,
    /// Prompt texts answered during the current job.
    pub answered: Vec<String>,
    /// Hosts Glowfly supplied credentials for during the current job.
    pub answered_hosts: HashSet<String>,
    /// Hosts whose last job failed after Glowfly supplied credentials (kept across jobs).
    pub failed_hosts: HashSet<String>,
    pub ended: Option<PromptEnd>,
    /// The prompt that ended the job early asked for a signing passphrase.
    pub ended_signing: bool,
    /// An SSH signing passphrase was given during the current job, so a failure that ssh-keygen
    /// calls a wrong passphrase gets its own headline.
    pub answered_ssh_signing: bool,
    /// View state: the dialog's "Remember this sign-in" box, set from the setting whenever
    /// prompts start queueing.
    pub remember: bool,
    /// HTTPS sign-ins typed during the current job with Remember on, saved or dropped through
    /// git's credential helper when it ends (update/askpass_store.rs).
    pub typed: Vec<HttpsCredential>,
}

impl CredentialPrompts {
    pub fn showing(&self) -> bool {
        !self.queue.is_empty()
    }

    pub fn front(&self) -> Option<&PendingPrompt> {
        self.queue.front()
    }

    /// Whether `prompt` is git's password prompt for the host and user just entered, so the
    /// kept password answers it.
    pub fn carry_answers(&self, prompt: &Prompt) -> bool {
        let Some(c) = &self.carry else { return false };
        let PromptKind::Password { user: Some(user), .. } = &prompt.kind else { return false };
        *user == c.user && askpass::host(&prompt.kind).is_some_and(|h| h == c.host)
    }

    /// Whether the shown prompt can be answered with what is typed.
    pub fn can_submit(&self) -> bool {
        match self.front().map(|p| &p.prompt.kind) {
            None => false,
            Some(PromptKind::Username { .. }) => !self.user.trim().is_empty(),
            Some(PromptKind::Confirm | PromptKind::HostKey(_) | PromptKind::Other { secret: false }) => true,
            Some(_) => !self.secret.is_empty(),
        }
    }

    /// Empties the text buffers (overwriting the secret) for the next prompt.
    pub fn clear_buffers(&mut self) {
        askpass::wipe(&mut self.user);
        askpass::wipe(&mut self.secret);
        self.reveal = false;
        self.focus_pending = true;
    }
}

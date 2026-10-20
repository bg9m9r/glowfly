//! Preferences > SSH: what a key card marks about its key (the default key, the one the open
//! repository uses, a key ssh tries on its own, a key the agent holds), which keys came from
//! outside the .ssh folder, and the state the Agent section's pill shows. Pure: the view words
//! them.

use super::ssh::SshPage;
use super::ssh_use::chosen_at;
use trellis_core::paths::same_spelling;
use trellis_core::ssh_keys::{AgentReach, AgentStatus, KeyChoice, SshKeyPair, SshListing, WinService, is_default_identity};

/// One mark on a key card.
#[derive(Clone, Debug, PartialEq)]
pub enum KeyMark {
    /// core.sshCommand names it for all repositories.
    Default,
    /// The open repository's own core.sshCommand names it (the repository's name).
    Repo(String),
    /// Nothing is chosen for all repositories, and ssh tries this key by its file name.
    TriedByDefault,
    /// The running agent holds it.
    InAgent,
}

/// The marks of `key`, in the order the card shows them.
pub fn key_marks(page: &SshPage, listing: &SshListing, key: &SshKeyPair) -> Vec<KeyMark> {
    let mut marks = Vec::new();
    for repo in chosen_at(page.choice.as_ref(), &key.private) {
        match (repo, &page.repo) {
            (false, _) => marks.push(KeyMark::Default),
            (true, Some(r)) => marks.push(KeyMark::Repo(r.name.clone())),
            (true, None) => {}
        }
    }
    let unset = page.choice.as_ref().and_then(|c| c.at(false)) == Some(&KeyChoice::Unset);
    if unset && is_default_identity(key, &listing.dir) {
        marks.push(KeyMark::TriedByDefault);
    }
    if key.fingerprint.as_deref().is_some_and(|fp| page.agent.as_ref().is_some_and(|a| a.holds(fp))) {
        marks.push(KeyMark::InAgent);
    }
    marks
}

/// Whether `key` lives outside the .ssh folder (it was added with Use existing).
pub fn outside_folder(key: &SshKeyPair, listing: &SshListing) -> bool {
    !key.private.parent().is_some_and(|p| same_spelling(p, &listing.dir))
}

/// What the Agent section's pill says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentPill {
    /// The check is running.
    Checking,
    /// An agent answered, holding this many keys.
    Running(usize),
    /// Something points at an agent (or ssh-add is missing or failed), but nothing answered.
    Unreachable,
    /// No agent, and nothing pointing at one.
    Off,
}

/// The pill for the last agent check (None while it runs). ssh-add can't reach Pageant, and
/// a set SSH_AUTH_SOCK or a running Windows service with no answer means the agent is there
/// but out of reach.
pub fn agent_pill(a: Option<&AgentStatus>) -> AgentPill {
    let Some(a) = a else { return AgentPill::Checking };
    if a.ssh_add.is_none() {
        return AgentPill::Unreachable;
    }
    match &a.reach {
        Some(AgentReach::Keys(keys)) => AgentPill::Running(keys.len()),
        Some(AgentReach::Failed(_)) => AgentPill::Unreachable,
        Some(AgentReach::NoAgent) | None => {
            if a.auth_sock_set || a.service == Some(WinService::Running) || a.pageant {
                AgentPill::Unreachable
            } else {
                AgentPill::Off
            }
        }
    }
}

#[cfg(test)]
#[path = "ssh_marks_tests.rs"]
mod tests;

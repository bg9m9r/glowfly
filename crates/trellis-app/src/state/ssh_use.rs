//! Preferences > SSH: the key git uses (core.sshCommand, globally or for the open repository)
//! and the local SSH agent. The rules that decide what may run now, and the page's texts.

use super::ssh::SshPage;
use super::trust::shown_value;
use std::path::{Path, PathBuf};
use trellis_core::paths::same_spelling;
use trellis_core::ssh_keys::{AgentReach, AgentStatus, KeyChoice, WinService};

pub const USE_AGENT_HELP: &str = "ssh gets keys from your running agent. Trellis leaves core.sshCommand unset while this is on.";
pub const NOT_TRUSTED: &str =
    "Trust this repository first: a key setting in its own config makes git run ssh with it, so Trellis would ask about trusting it.";
pub const AGENT_ON: &str = "Turn off Use local SSH agent to choose a key";
pub const NOT_SET: &str = "Not set: ssh picks the key (your ssh config, the agent, or the default key names)";
pub const OTHER_WITH_AGENT: &str = "git runs this command instead of plain ssh; it may not use the agent";
pub const GIT_SSH_COMMAND_SET: &str =
    "GIT_SSH_COMMAND is set in Trellis's environment: it overrides core.sshCommand for every git command Trellis runs.";
pub const AGENT_TOAST: &str = "Trellis leaves core.sshCommand unset; ssh uses your agent.";
pub const WINDOWS_AGENT_NOTE: &str = "Git for Windows' own ssh doesn't read keys from the Windows agent service; git uses it unless GIT_SSH or core.sshCommand points at Windows' ssh.exe.";
pub const PAGEANT_NOTE: &str = "Pageant is running. Add keys in Pageant; ssh-add can't reach it.";
pub const NO_AUTH_SOCK: &str = "SSH_AUTH_SOCK isn't set in Trellis's environment; start ssh-agent in your login session.";
pub const NO_SSH_ADD: &str = "ssh-add wasn't found. Install OpenSSH (or Git for Windows) and reload.";
pub const LOOKING: &str = "Looking for an SSH agent…";
pub const REPLACE_COMMAND_TITLE: &str = "Replace SSH command";
pub const REMOVE_COMMAND_TITLE: &str = "Remove SSH command";

/// core.sshCommand at each level, as last read.
#[derive(Clone, Debug, PartialEq)]
pub struct Choices {
    pub global: Result<KeyChoice, String>,
    /// None without an open repository.
    pub repo: Option<Result<KeyChoice, String>>,
}

impl Choices {
    /// The choice at one level, if it was read.
    pub fn at(&self, repo: bool) -> Option<&KeyChoice> {
        if repo { self.repo.as_ref()?.as_ref().ok() } else { self.global.as_ref().ok() }
    }
}

/// The repository whose own core.sshCommand the page offers to set.
#[derive(Clone, Debug, PartialEq)]
pub struct RepoTarget {
    pub name: String,
    pub workdir: PathBuf,
    /// Where it really is (`Snapshot::real_workdir`), for its trust decision.
    pub real: PathBuf,
    /// The user chose to trust it (`Standing::Trusted`).
    pub trusted: bool,
}

/// Why Use for git (or Stop using) at this level can't run now.
pub fn use_blocked(page: &SshPage, use_agent: bool, repo: bool) -> Option<&'static str> {
    if use_agent {
        return Some(AGENT_ON);
    }
    if repo {
        let Some(target) = &page.repo else { return Some("Open a repository first.") };
        if !target.trusted {
            return Some(NOT_TRUSTED);
        }
    }
    if page.choice_busy {
        return Some("core.sshCommand is being changed.");
    }
    if page.choice.as_ref().and_then(|c| c.at(repo)).is_none() {
        return Some("Wait until core.sshCommand has been read.");
    }
    None
}

/// Why Add to agent can't run now; `job` is the running git job's label.
pub fn add_blocked(page: &SshPage, job: Option<&str>) -> Option<String> {
    let Some(agent) = &page.agent else { return Some("Wait until the agent has been checked.".into()) };
    if agent.ssh_add.is_none() {
        return Some("ssh-add wasn't found.".into());
    }
    if !agent.running() {
        return Some("No SSH agent is running.".into());
    }
    if page.agent_adding.is_some() {
        return Some("A key is being added to the agent.".into());
    }
    job.map(|label| format!("Wait until {label} finishes: its credential prompts would mix with ssh-add's."))
}

/// A level, as toasts and dialogs name it: "all repositories" or "<repo> only".
pub fn level_label(repo: Option<&str>) -> String {
    match repo {
        Some(name) => format!("{name} only"),
        None => "all repositories".into(),
    }
}

/// The levels (`false` global, `true` the repository) whose choice is `key` in Trellis's form.
pub fn chosen_at(choices: Option<&Choices>, key: &Path) -> Vec<bool> {
    let Some(c) = choices else { return Vec::new() };
    [false, true].into_iter().filter(|&repo| matches!(c.at(repo), Some(KeyChoice::Key(p)) if same_spelling(p, key))).collect()
}

pub fn replace_command_body(level: &str, value: &str) -> String {
    format!(
        "core.sshCommand for {level} is \u{201c}{}\u{201d}, which Trellis didn't write. Replace it with the chosen key?",
        shown_value(value)
    )
}

pub fn remove_command_body(level: &str, value: &str) -> String {
    format!(
        "core.sshCommand for {level} is \u{201c}{}\u{201d}, which Trellis didn't write. Remove it, so git runs plain ssh?",
        shown_value(value)
    )
}

/// The Windows agent service's state, for the agent row.
pub fn service_line(s: &WinService) -> &'static str {
    match s {
        WinService::Running => "The OpenSSH Authentication Agent service is running",
        WinService::Stopped => "The OpenSSH Authentication Agent service is stopped",
        WinService::Disabled => "The OpenSSH Authentication Agent service is disabled (an administrator can enable it in Services)",
        WinService::Missing => "The OpenSSH Authentication Agent service isn't installed",
    }
}

/// The agent row's status line.
pub fn agent_summary(a: &AgentStatus) -> String {
    if a.ssh_add.is_none() {
        return NO_SSH_ADD.into();
    }
    match &a.reach {
        Some(AgentReach::Keys(keys)) if keys.len() == 1 => "Agent running: 1 key loaded".into(),
        Some(AgentReach::Keys(keys)) if !keys.is_empty() => format!("Agent running: {} keys loaded", keys.len()),
        Some(AgentReach::Keys(_)) => "Agent running, no keys loaded".into(),
        Some(AgentReach::NoAgent) | None => "No SSH agent is running".into(),
        Some(AgentReach::Failed(e)) => format!("Couldn't ask the SSH agent: {e}"),
    }
}

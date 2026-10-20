//! Preferences > SSH: the key git uses (core.sshCommand, globally or for the open repository)
//! and the local SSH agent. The rules that decide what may run now, and the texts update's
//! toasts and confirmations show (the views take theirs from the language tables).

use super::ssh::SshPage;
use super::trust::shown_value;
use std::path::{Path, PathBuf};
use trellis_core::paths::same_spelling;
use trellis_core::ssh_keys::KeyChoice;

pub const NOT_TRUSTED: &str =
    "Trust this repository first: a key setting in its own config makes git run ssh with it, so Trellis would ask about trusting it.";
pub const AGENT_ON: &str = "Turn off Use local SSH agent to choose a key";
pub const AGENT_TOAST: &str = "Trellis leaves core.sshCommand unset; ssh uses your agent.";
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

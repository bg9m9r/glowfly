//! Preferences > SSH: the key git uses (core.sshCommand, globally or for the open repository)
//! and the local SSH agent. The rules that decide what may run now, and the texts update's
//! toasts and confirmations show (the views take theirs from the language tables).

use super::ssh::SshPage;
use super::trust::shown_value;
use glowfly_core::i18n::Lang;
use glowfly_core::paths::same_spelling;
use glowfly_core::ssh_keys::KeyChoice;
use std::path::{Path, PathBuf};

pub const AGENT_TOAST: &str = "Glowfly doesn't choose a key in core.sshCommand; ssh uses your agent.";
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

/// Why Use for git (or Stop using) at this level can't run now, in `t`.
pub fn use_blocked(page: &SshPage, use_agent: bool, repo: bool, t: Lang) -> Option<&'static str> {
    if use_agent {
        return Some(t.tr("prefs.ssh.which.agent_on.tip"));
    }
    if repo {
        let Some(target) = &page.repo else { return Some(t.tr("prefs.ssh.which.no_repo.tip")) };
        if !target.trusted {
            return Some(not_trusted(t));
        }
    }
    if page.choice_busy {
        return Some(t.tr("prefs.ssh.which.busy.tip"));
    }
    if page.choice.as_ref().and_then(|c| c.at(repo)).is_none() {
        return Some(t.tr("prefs.ssh.which.reading.tip"));
    }
    None
}

/// Why a key can't be chosen for an untrusted repository, in `t`.
pub fn not_trusted(t: Lang) -> &'static str {
    t.tr("prefs.ssh.which.not_trusted.tip")
}

/// Why Add to agent can't run now, in `t`; `job` is the running git job's label.
pub fn add_blocked(page: &SshPage, job: Option<&str>, t: Lang) -> Option<String> {
    let Some(agent) = &page.agent else { return Some(t.tr("prefs.ssh.card.add_agent.checking.tip").into()) };
    if agent.ssh_add.is_none() {
        return Some(t.tr("prefs.ssh.card.add_agent.no_ssh_add.tip").into());
    }
    if !agent.running() {
        return Some(t.tr("prefs.ssh.card.add_agent.no_agent.tip").into());
    }
    if page.agent_adding.is_some() {
        return Some(t.tr("prefs.ssh.card.add_agent.busy.tip").into());
    }
    job.map(|label| t.trf("prefs.ssh.card.add_agent.job.tip", &[("job", &label)]))
}

/// A level, as toasts and dialogs name it: "all repositories" or "<repo> only".
pub fn level_label(repo: Option<&str>) -> String {
    match repo {
        Some(name) => format!("{name} only"),
        None => "all repositories".into(),
    }
}

/// The levels (`false` global, `true` the repository) whose choice is `key` in Glowfly's form.
pub fn chosen_at(choices: Option<&Choices>, key: &Path) -> Vec<bool> {
    let Some(c) = choices else { return Vec::new() };
    [false, true].into_iter().filter(|&repo| matches!(c.at(repo), Some(KeyChoice::Key(p)) if same_spelling(p, key))).collect()
}

pub fn replace_command_body(level: &str, value: &str) -> String {
    format!(
        "core.sshCommand for {level} is \u{201c}{}\u{201d}, which Glowfly didn't write. Replace it with the chosen key?",
        shown_value(value)
    )
}

pub fn remove_command_body(level: &str, value: &str) -> String {
    format!(
        "core.sshCommand for {level} is \u{201c}{}\u{201d}, which Glowfly didn't write. Remove it, so git runs plain ssh?",
        shown_value(value)
    )
}

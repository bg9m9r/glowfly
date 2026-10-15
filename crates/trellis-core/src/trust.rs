//! What in a repository's own config and hooks would make git run programs (repository trust).
//!
//! A repository from elsewhere can name programs in its `.git/config` (an ssh command, a
//! credential helper, filters, an fsmonitor hook) or carry hooks, and git runs them as soon as it
//! touches the repository. `scan` lists them with libgit2 only, so reading them runs nothing.

use git2::{Config, ConfigLevel, Repository};
use std::path::{Path, PathBuf};

/// A setting from the repository's own config that makes git run a program.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConfigCommand {
    /// The key in its usual spelling, e.g. `core.sshCommand`.
    pub key: String,
    pub value: String,
}

/// What `scan` found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrustScan {
    /// Settings from the repository's own config files (local and worktree level, with what they
    /// include), in file order.
    pub config: Vec<ConfigCommand>,
    /// Hook names that git would run, sorted.
    pub hooks: Vec<String>,
    /// The folder `hooks` were found in; None when hooks come from the user's own (global or
    /// system) core.hooksPath, or from a path that can't be resolved without the shell.
    pub hooks_dir: Option<PathBuf>,
}

impl TrustScan {
    /// Nothing in the repository makes git run a program.
    pub fn is_empty(&self) -> bool {
        self.config.is_empty() && self.hooks.is_empty()
    }
}

/// The hook names git runs (`git help hooks`). Anything else in the folder, such as the
/// `*.sample` files `git init` writes, is never run.
pub const HOOK_NAMES: &[&str] = &[
    "applypatch-msg",
    "pre-applypatch",
    "post-applypatch",
    "pre-commit",
    "pre-merge-commit",
    "prepare-commit-msg",
    "commit-msg",
    "post-commit",
    "pre-rebase",
    "post-checkout",
    "post-merge",
    "pre-push",
    "pre-receive",
    "update",
    "proc-receive",
    "post-receive",
    "post-update",
    "reference-transaction",
    "push-to-checkout",
    "pre-auto-gc",
    "post-rewrite",
    "sendemail-validate",
    "fsmonitor-watchman",
    "p4-changelist",
    "p4-prepare-changelist",
    "p4-post-changelist",
    "p4-pre-submit",
    "post-index-change",
];

/// Single keys that name a program, as libgit2 returns them (lowercase), with their usual spelling.
const COMMAND_KEYS: &[(&str, &str)] = &[
    ("core.sshcommand", "core.sshCommand"),
    ("core.hookspath", "core.hooksPath"),
    ("core.editor", "core.editor"),
    ("core.askpass", "core.askPass"),
    ("core.gitproxy", "core.gitProxy"),
    ("sequence.editor", "sequence.editor"),
    ("diff.external", "diff.external"),
    ("credential.helper", "credential.helper"),
    ("gpg.program", "gpg.program"),
    ("gpg.openpgp.program", "gpg.openpgp.program"),
    ("gpg.x509.program", "gpg.x509.program"),
    ("gpg.ssh.program", "gpg.ssh.program"),
    ("gpg.ssh.defaultkeycommand", "gpg.ssh.defaultKeyCommand"),
];

/// Lists what in `repo`'s own config and hooks would make git run programs. Settings from the
/// user's own config (global, XDG, system) are left out: the user put them there.
pub fn scan(repo: &Repository) -> TrustScan {
    let mut out = TrustScan::default();
    let config = repo.config().ok();
    let mut local_hooks_path = None;
    if let Some(entries) = config.as_ref().and_then(|c| c.entries(None).ok()) {
        let _ = entries.for_each(|e| {
            if !matches!(e.level(), ConfigLevel::Local | ConfigLevel::Worktree) {
                return;
            }
            let (Some(name), Some(value)) = (e.name(), e.value()) else { return };
            if let Some(key) = command_key(name, value) {
                if name == "core.hookspath" {
                    local_hooks_path = Some(value.to_owned());
                }
                out.config.push(ConfigCommand { key, value: value.to_owned() });
            }
        });
    }
    out.hooks_dir = hooks_dir(repo, config.as_ref(), local_hooks_path);
    if let Some(dir) = &out.hooks_dir {
        out.hooks = active_hooks(dir);
    }
    out
}

/// `scan` of the repository at `path`.
pub fn scan_at(path: &Path) -> crate::error::Result<TrustScan> {
    Ok(scan(&Repository::open(path)?))
}

/// Whether `git init` output says it made a new repository (not "Reinitialized existing").
pub fn init_created_new(output: &str) -> bool {
    output.lines().any(|l| l.contains("Initialized empty Git repository"))
}

/// The display key when the setting `name` = `value` makes git run a program.
fn command_key(name: &str, value: &str) -> Option<String> {
    // An empty value resets a list (credential.helper) or turns the setting off.
    if value.is_empty() {
        return None;
    }
    if let Some((_, shown)) = COMMAND_KEYS.iter().find(|(k, _)| *k == name) {
        return Some((*shown).to_owned());
    }
    if name == "core.fsmonitor" {
        // A boolean means git's built-in daemon; anything else is a hook's path.
        return Config::parse_bool(value).is_err().then(|| name.to_owned());
    }
    let (first, last) = (name.find('.')?, name.rfind('.')?);
    if first == last {
        return None;
    }
    let (section, key) = (&name[..first], &name[last + 1..]);
    let runs = match section {
        "credential" => key == "helper",
        "diff" => matches!(key, "command" | "textconv"),
        "merge" => key == "driver",
        "filter" => matches!(key, "clean" | "smudge" | "process"),
        "difftool" | "mergetool" => key == "cmd",
        "remote" => matches!(key, "uploadpack" | "receivepack") || (matches!(key, "url" | "pushurl") && value.starts_with("ext::")),
        _ => false,
    };
    runs.then(|| name.to_owned())
}

/// The folder git takes hooks from. `local` is the repository's own core.hooksPath, if set.
fn hooks_dir(repo: &Repository, config: Option<&Config>, local: Option<String>) -> Option<PathBuf> {
    if let Some(v) = local {
        // A home-relative path needs the shell's idea of home: report the key alone.
        if v.starts_with('~') {
            return None;
        }
        let p = PathBuf::from(v);
        if p.is_absolute() {
            return Some(p);
        }
        return Some(repo.workdir().unwrap_or_else(|| repo.path()).join(p));
    }
    // Set in the user's own config: their hooks, not the repository's.
    if config.is_some_and(|c| c.get_path("core.hooksPath").is_ok()) {
        return None;
    }
    // A linked worktree runs its main repository's hooks.
    Some(repo.commondir().join("hooks"))
}

/// The hooks in `dir` that git would run, sorted. A missing folder has none.
fn active_hooks(dir: &Path) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut hooks: Vec<String> = read
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_owned();
            if !HOOK_NAMES.contains(&name.as_str()) {
                return None;
            }
            // Follows links: a link to a script runs the script.
            let meta = std::fs::metadata(e.path()).ok()?;
            (meta.is_file() && runnable(&meta)).then_some(name)
        })
        .collect();
    hooks.sort();
    hooks
}

/// Git skips hooks without an execute bit on Unix; on Windows it runs any file.
#[cfg(unix)]
fn runnable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn runnable(_meta: &std::fs::Metadata) -> bool {
    true
}

#[cfg(test)]
mod tests;

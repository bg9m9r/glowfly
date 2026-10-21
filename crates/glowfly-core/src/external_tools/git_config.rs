//! The diff and merge tools git config names, for when Preferences > External tools sets none:
//! `diff.tool` (falling back to `merge.tool`) or `merge.tool` picks a tool by name, whose
//! `difftool.<name>.cmd` / `mergetool.<name>.cmd` is a command to run in sh; without one, a
//! name git knows built in (a file in `$(git --exec-path)/mergetools`) runs through
//! `git difftool` / `git mergetool`. The fallbacks are git's own (git-mergetool--lib), and the
//! values that come from the repository's own config are told apart, since running those needs
//! the repository's trust. Parsing and resolving are pure; [`read`] runs git.

use crate::gitcli;
use std::cell::OnceCell;
use std::path::{Path, PathBuf};

/// The keys read, as `git config --get-regexp` matches them (section and variable lowercased).
pub const KEYS: &str = r"^(diff|merge)\.tool$|^(diff|merge)tool\..+\.(cmd|path|trustexitcode)$";

/// A config entry: its key and value; None for a key written without `=`.
pub type Entry = (String, Option<String>);

/// Reads `git config -z --get-regexp` output: entries end in NUL, each `key\nvalue` (the value
/// may hold newlines) or a bare `key` for a valueless entry.
pub fn parse_z(out: &str) -> Vec<Entry> {
    out.split('\0')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('\n') {
            Some((k, v)) => (k.to_owned(), Some(v.to_owned())),
            None => (p.to_owned(), None),
        })
        .collect()
}

/// Which of git's two tools: `git difftool` or `git mergetool`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolMode {
    Diff,
    Merge,
}

impl ToolMode {
    /// "diff" or "merge".
    pub fn word(self) -> &'static str {
        match self {
            Self::Diff => "diff",
            Self::Merge => "merge",
        }
    }

    /// The git command that runs a tool: "difftool" or "mergetool".
    pub fn command(self) -> &'static str {
        match self {
            Self::Diff => "difftool",
            Self::Merge => "mergetool",
        }
    }

    /// The keys that choose the tool, in the order git tries them.
    pub fn selectors(self) -> &'static [&'static str] {
        match self {
            Self::Diff => &["diff.tool", "merge.tool"],
            Self::Merge => &["merge.tool"],
        }
    }

    /// The sections holding a tool's `cmd` and `path`, in the order git tries them.
    pub fn sections(self) -> &'static [&'static str] {
        match self {
            Self::Diff => &["difftool", "mergetool"],
            Self::Merge => &["mergetool"],
        }
    }
}

/// The tool git config names for one mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitTool {
    pub name: String,
    /// The key that named it: "diff.tool" or "merge.tool".
    pub chosen_by: String,
    pub run: GitToolRun,
}

/// How a git config tool runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitToolRun {
    /// `key` (e.g. `difftool.meld.cmd`) holds a command for sh. `trust_exit_code` is
    /// `mergetool.<name>.trustExitCode` (merge only); `from_repo` that the command is the
    /// repository's own value.
    Command { key: String, cmd: String, trust_exit_code: bool, from_repo: bool },
    /// A tool git knows built in, run by `git difftool` / `git mergetool`; `path` is its
    /// `<section>.<name>.path`, when set.
    BuiltIn { path: Option<String> },
    /// No command and no built-in tool by that name: it can't be used.
    Unknown,
}

/// git config's diff and merge tools; None where it names none.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GitTools {
    pub diff: Option<GitTool>,
    pub merge: Option<GitTool>,
}

impl GitTool {
    /// Whether running it needs the repository's trust: its command is the repository's own,
    /// or it is built in (`git difftool` / `git mergetool` run git in the repository).
    pub fn needs_trust(&self) -> bool {
        matches!(self.run, GitToolRun::BuiltIn { .. } | GitToolRun::Command { from_repo: true, .. })
    }

    /// The keys that could hold its command in `mode`, in git's order (for messages).
    pub fn cmd_keys(&self, mode: ToolMode) -> Vec<String> {
        mode.sections().iter().map(|s| format!("{s}.{}.cmd", self.name)).collect()
    }
}

/// The last entry for exactly `key`: None when it is unset, Some(None) when valueless.
fn last<'a>(e: &'a [Entry], key: &str) -> Option<Option<&'a str>> {
    e.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v.as_deref())
}

/// A git boolean as `git config --bool` reads it: unset or invalid is false, a valueless key
/// true.
fn git_bool(v: Option<Option<&str>>) -> bool {
    match v {
        None => false,
        Some(None) => true,
        Some(Some(s)) => git2::Config::parse_bool(s).unwrap_or(false),
    }
}

/// The tool git config names for `mode`, from every scope's entries (`all`, in git's order)
/// and the repository's own (`own`: its local and worktree config, with what they include).
/// `builtin` says whether git knows a tool by that name.
pub fn resolve(all: &[Entry], own: &[Entry], mode: ToolMode, builtin: &dyn Fn(&str) -> bool) -> Option<GitTool> {
    // `git config <key>` gives the last value, used when non-empty (`test -n`).
    let (chosen_by, name) = mode.selectors().iter().find_map(|k| match last(all, k) {
        Some(Some(v)) if !v.is_empty() => Some((*k, v)),
        _ => None,
    })?;
    // `git config difftool.x.cmd || git config mergetool.x.cmd`: only an unset key falls back.
    let first_set =
        |var: &str| mode.sections().iter().map(|s| format!("{s}.{name}.{var}")).find_map(|k| last(all, &k).map(|v| (k, v.unwrap_or(""))));
    let run = match first_set("cmd") {
        Some((key, cmd)) if !cmd.is_empty() => {
            let trust_exit_code = mode == ToolMode::Merge && git_bool(last(all, &format!("mergetool.{name}.trustexitcode")));
            let mine = last(own, &key);
            let from_repo = mine.is_some() && mine == last(all, &key);
            GitToolRun::Command { key, cmd: cmd.to_owned(), trust_exit_code, from_repo }
        }
        _ if builtin(name) => {
            let path = first_set("path").map(|(_, p)| p).filter(|p| !p.is_empty()).map(str::to_owned);
            GitToolRun::BuiltIn { path }
        }
        _ => GitToolRun::Unknown,
    };
    Some(GitTool { name: name.to_owned(), chosen_by: chosen_by.to_owned(), run })
}

/// Whether `dir` (git's mergetools folder) has a tool called `name`, or `name` without one
/// trailing digit (`vimdiff3` is a variant of `vimdiff`). Names that aren't a plain file name
/// never match.
pub fn builtin_in(dir: &Path, name: &str) -> bool {
    if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
        return false;
    }
    let base = name.strip_suffix(|c: char| c.is_ascii_digit()).filter(|b| !b.is_empty());
    dir.join(name).is_file() || base.is_some_and(|b| dir.join(b).is_file())
}

/// Reads git config's diff and merge tools in `cwd`; with `has_repo`, also the repository's
/// own values (`--local` and `--worktree`, with their includes). `env` is added to the
/// commands' environment. Nothing is run but `git config` and `git --exec-path`. The error is
/// git's first line. Runner threads only.
pub fn read(cwd: &Path, has_repo: bool, env: &[(&str, &str)]) -> Result<GitTools, String> {
    let vars: Vec<(&str, &str)> = gitcli::NO_PROMPTS.iter().chain(env).copied().collect();
    let run = |a: &[&str]| gitcli::output(cwd, &gitcli::args(a), "", &vars, Some(gitcli::READ_TIMEOUT));
    let get = |scope: &[&str]| -> Result<Vec<Entry>, String> {
        let a: Vec<&str> = ["config"].iter().chain(scope).chain(&["-z", "--get-regexp", KEYS]).copied().collect();
        let out = run(&a);
        match out.code {
            Some(0) => Ok(parse_z(&out.stdout)),
            // No key matched.
            Some(1) => Ok(Vec::new()),
            _ => Err(out.stderr.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("git config failed").to_owned()),
        }
    };
    let all = get(&[])?;
    let mut own = Vec::new();
    if has_repo {
        own = get(&["--local", "--includes"])?;
        // Fails in a linked worktree unless extensions.worktreeConfig is on: nothing of its own.
        if let Ok(w) = get(&["--worktree", "--includes"]) {
            own.extend(w);
        }
    }
    let dir: OnceCell<Option<PathBuf>> = OnceCell::new();
    let builtin = |name: &str| {
        let dir = dir.get_or_init(|| {
            let out = run(&["--exec-path"]);
            let p = out.stdout.trim();
            (out.code == Some(0) && !p.is_empty()).then(|| PathBuf::from(p).join("mergetools"))
        });
        // Unreadable: git says what is wrong itself when it runs the tool.
        dir.as_deref().is_none_or(|d| builtin_in(d, name))
    };
    Ok(GitTools { diff: resolve(&all, &own, ToolMode::Diff, &builtin), merge: resolve(&all, &own, ToolMode::Merge, &builtin) })
}

#[cfg(test)]
#[path = "git_config_tests.rs"]
mod tests;

//! The stash Trellis makes before an operation that needs the uncommitted changes out of the way,
//! and putting it back afterwards.
//!
//! The stash's message is fixed English text kept in the repository ("Auto-stash before checking
//! out feature"), so the STASHES list can recognise it whatever language the UI shows. It is
//! made with `git stash push -m` in the same job as the operation (untracked files stay, as with
//! git's own `--autostash`) and put back with a plain `git stash pop`: staged changes come back
//! unstaged, and on conflicts git keeps the entry.

use super::{list, split_message};
use crate::gitcli::{self, args};
use crate::undo::{commits_label, move_targets};
use git2::{Oid, RepositoryState};
use std::path::Path;

/// What every auto-stash message starts with.
pub const AUTO_STASH_PREFIX: &str = "Auto-stash before ";
/// The list text of the entry git keeps when its own `--autostash` can't be applied.
pub const GIT_AUTOSTASH: &str = "autostash";

/// The operation an auto-stash was made for, with the names its message shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AutoStashFor {
    Checkout { target: String },
    Pull { branch: String },
    Merge { from: String, into: String },
    Rebase { branch: String, onto: String },
    CherryPick { commits: String, onto: String },
    Revert { commits: String, on: String },
    HardReset { branch: String, to: String },
}

/// Flags that carry on, end or edit an operation already started, or make a rebase interactive.
const NOT_GUARDED: &[&str] = &["--abort", "--continue", "--skip", "--quit", "--edit-todo", "-i", "--interactive"];

impl AutoStashFor {
    /// The operation `argv` (one git command line, without "git") starts, when it is one the
    /// auto-stash guards; `head` is the checked-out branch (None in detached HEAD).
    pub fn of_command(argv: &[String], head: Option<&str>) -> Option<Self> {
        let a: Vec<&str> = argv.iter().map(String::as_str).collect();
        let head = head.unwrap_or("HEAD").to_owned();
        match a.as_slice() {
            ["checkout", rest @ ..] => checkout_target(rest).map(|target| Self::Checkout { target }),
            ["reset", "--hard", rev] if !rev.starts_with('-') => Some(Self::HardReset { branch: head, to: short(rev) }),
            [op @ ("pull" | "merge" | "rebase" | "cherry-pick" | "revert"), rest @ ..] => {
                if rest.iter().any(|f| NOT_GUARDED.contains(f)) {
                    return None;
                }
                match *op {
                    "pull" => Some(Self::Pull { branch: head }),
                    "merge" => first_target(rest).map(|from| Self::Merge { from: short(from), into: head }),
                    "rebase" => {
                        let onto = rest.iter().position(|f| *f == "--onto").and_then(|i| rest.get(i + 1)).copied();
                        onto.or_else(|| first_target(rest)).map(|o| Self::Rebase { branch: head, onto: short(o) })
                    }
                    "cherry-pick" => commits_label(&move_targets(rest, true)).map(|commits| Self::CherryPick { commits, onto: head }),
                    _ => commits_label(&move_targets(rest, true)).map(|commits| Self::Revert { commits, on: head }),
                }
            }
            _ => None,
        }
    }

    /// The stash's message: fixed English text, never translated, starting with
    /// `AUTO_STASH_PREFIX`.
    pub fn message(&self) -> String {
        let what = match self {
            Self::Checkout { target } => format!("checking out {target}"),
            Self::Pull { branch } => format!("pulling into {branch}"),
            Self::Merge { from, into } => format!("merging {from} into {into}"),
            Self::Rebase { branch, onto } => format!("rebasing {branch} onto {onto}"),
            Self::CherryPick { commits, onto } => format!("cherry-picking {commits} onto {onto}"),
            Self::Revert { commits, on } => format!("reverting {commits} on {on}"),
            Self::HardReset { branch, to } => format!("resetting {branch} to {to}"),
        };
        format!("{AUTO_STASH_PREFIX}{what}")
    }

    /// `git stash push -m <message>`, run first in the operation's job.
    pub fn stash_command(&self) -> Vec<String> {
        args(&["stash", "push", "-m", &self.message()])
    }

    /// Whether the operation can stop on conflicts and wait for Continue or Abort.
    pub fn can_stop(&self) -> bool {
        matches!(self, Self::Pull { .. } | Self::Merge { .. } | Self::Rebase { .. } | Self::CherryPick { .. } | Self::Revert { .. })
    }

    pub fn is_hard_reset(&self) -> bool {
        matches!(self, Self::HardReset { .. })
    }
}

/// The target of `checkout <target>` (with `--track`, `--detach` or a trailing `--`), or the
/// new branch of `checkout -b <name> <start>`. `-b` without a start point changes no files, and
/// a `--` before paths restores files: neither is guarded.
fn checkout_target(rest: &[&str]) -> Option<String> {
    let mut targets = Vec::new();
    let mut new_branch = None;
    let mut it = rest.iter().enumerate();
    while let Some((i, arg)) = it.next() {
        match *arg {
            "--track" | "--detach" => {}
            "-b" => match it.next() {
                Some((_, name)) if !name.starts_with('-') && new_branch.is_none() => new_branch = Some(*name),
                _ => return None,
            },
            "--" if i + 1 == rest.len() => {}
            s if s.starts_with('-') => return None,
            s => targets.push(s),
        }
    }
    match (new_branch, targets.as_slice()) {
        (Some(name), [_start]) => Some(name.to_owned()),
        (None, [target]) => Some(short(target)),
        _ => None,
    }
}

/// The first argument that isn't a flag, skipping the values of merge's `-m`, `-F`, `-s` and `-X`.
fn first_target<'a>(rest: &[&'a str]) -> Option<&'a str> {
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match *a {
            "-m" | "-F" | "-s" | "-X" | "--onto" => {
                it.next();
            }
            s if !s.starts_with('-') => return Some(s),
            _ => {}
        }
    }
    None
}

/// A revision as the message shows it: without `refs/heads/`, `refs/tags/` or `refs/remotes/`,
/// and a full commit id cut to 7 characters.
fn short(rev: &str) -> String {
    let rev = ["refs/heads/", "refs/tags/", "refs/remotes/"].iter().find_map(|p| rev.strip_prefix(p)).unwrap_or(rev);
    if rev.len() == 40 && rev.bytes().all(|b| b.is_ascii_hexdigit()) { rev[..7].to_owned() } else { rev.to_owned() }
}

/// Whether a stash list text was made by Trellis before an operation, or is git's own kept
/// "autostash".
pub fn is_auto_stash(list_text: &str) -> bool {
    split_message(list_text).1.starts_with(AUTO_STASH_PREFIX) || list_text == GIT_AUTOSTASH
}

/// How putting an auto-stash back went.
#[derive(Clone, Debug, PartialEq)]
pub enum PutBack {
    /// The changes are back and the entry is gone.
    Restored,
    /// git applied the stash with conflicts in `paths` (repository-relative, '/'-separated) and
    /// kept the entry.
    Conflicted { paths: Vec<String> },
    /// git refused or failed; the entry stays.
    Failed,
    /// The stash is no longer in the list (popped or dropped meanwhile).
    Gone,
    /// An operation is in progress or files are conflicted: try again later.
    NotReady,
}

/// Where stash `oid` is in the list (its n in `stash@{n}`), once the repository has nothing in
/// progress and no conflicted files.
pub fn prepare_put_back(workdir: &Path, oid: Oid) -> Result<usize, (PutBack, String)> {
    let failed = |e: git2::Error| (PutBack::Failed, format!("Could not read the repository: {e}"));
    let repo = git2::Repository::open(workdir).map_err(failed)?;
    let conflicts = repo.index().map_err(failed)?.has_conflicts();
    if repo.state() != RepositoryState::Clean || conflicts {
        return Err((PutBack::NotReady, "An operation is still in progress; the stashed changes wait until it ends.".to_owned()));
    }
    let entries = list(workdir).map_err(|e| (PutBack::Failed, format!("Could not read the stash list: {e}")))?;
    match entries.iter().find(|s| s.oid == oid) {
        Some(s) => Ok(s.index),
        None => Err((PutBack::Gone, "The stash is no longer in the list; nothing was put back.".to_owned())),
    }
}

/// Runs `git stash pop stash@{index}` (found by `prepare_put_back` for `oid`) and tells what
/// happened, with the command and its output for the log.
pub fn pop_put_back(workdir: &Path, index: usize, oid: Oid) -> (PutBack, String) {
    let cmd = args(&["stash", "pop", &format!("stash@{{{index}}}")]);
    let (ok, text) = gitcli::run(workdir, &cmd);
    let mut output = format!("$ git {}", cmd.join(" "));
    if !text.is_empty() {
        output.push('\n');
        output.push_str(text.trim_end());
    }
    let listed = list(workdir).map(|l| l.iter().any(|s| s.oid == oid)).unwrap_or(true);
    if ok && !listed {
        return (PutBack::Restored, output);
    }
    let paths = conflicted_paths(workdir);
    if !ok && listed && !paths.is_empty() {
        return (PutBack::Conflicted { paths }, output);
    }
    (PutBack::Failed, output)
}

/// The conflicted paths in the index, sorted.
fn conflicted_paths(workdir: &Path) -> Vec<String> {
    let Ok(index) = git2::Repository::open(workdir).and_then(|r| r.index()) else { return Vec::new() };
    let Ok(conflicts) = index.conflicts() else { return Vec::new() };
    let mut paths: Vec<String> = conflicts
        .flatten()
        .filter_map(|c| c.our.or(c.their).or(c.ancestor))
        .map(|e| String::from_utf8_lossy(&e.path).into_owned())
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

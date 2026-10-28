//! The stash Glowfly makes before an operation that needs the uncommitted changes out of the way,
//! and putting it back afterwards.
//!
//! The stash's message is fixed English text kept in the repository ("Auto-stash before checking
//! out feature"), so the STASHES list can recognise it whatever language the UI shows. It is
//! made with `git stash push -m` in the same job as the operation (untracked files stay, as with
//! git's own `--autostash`) and put back with `git stash pop --index`, so what was staged comes
//! back staged. When git can't restore the staged state (the staged hunks no longer fit the
//! index) it changes nothing, and a plain `git stash pop` follows, bringing everything back
//! unstaged. On conflicts git keeps the entry.

use super::{list, split_message};
use crate::gitcli::{self, args};
use crate::undo::{commits_label, move_targets};
use git2::{Oid, RepositoryState};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// What every auto-stash message starts with.
pub const AUTO_STASH_PREFIX: &str = "Auto-stash before ";
/// The list text of the entry git keeps when its own `--autostash` can't be applied.
pub const GIT_AUTOSTASH: &str = "autostash";

/// The operation an auto-stash was made for, with the names its message shows. Most come
/// from the command about to run (`of_command`). The others are asked for by their entry
/// points: a Git Flow start or finish (its full branch name, such as "feature/login"), the
/// checkout of a pull request's source branch (`pr` is the forge's label, such as "#12") and a
/// restack (`stack::restack` makes and puts back its stash itself).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AutoStashFor {
    Checkout {
        target: String,
    },
    Pull {
        branch: String,
    },
    Merge {
        from: String,
        into: String,
    },
    Rebase {
        branch: String,
        onto: String,
    },
    CherryPick {
        commits: String,
        onto: String,
    },
    Revert {
        commits: String,
        on: String,
    },
    HardReset {
        branch: String,
        to: String,
    },
    GitFlowFinish {
        branch: String,
    },
    GitFlowStart {
        branch: String,
    },
    PullRequestCheckout {
        pr: String,
        branch: String,
    },
    /// A restack of stacked branches onto `onto`, stashed once before its first step.
    Restack {
        onto: String,
    },
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
            Self::GitFlowFinish { branch } => format!("finishing {branch}"),
            Self::GitFlowStart { branch } => format!("starting {branch}"),
            Self::PullRequestCheckout { pr, branch } => format!("checking out {branch} for {pr}"),
            Self::Restack { onto } => format!("restacking onto {onto}"),
        };
        format!("{AUTO_STASH_PREFIX}{what}")
    }

    /// `git stash push -m <message>`, run first in the operation's job.
    pub fn stash_command(&self) -> Vec<String> {
        args(&["stash", "push", "-m", &self.message()])
    }

    /// Whether the operation can stop on conflicts and wait for Continue or Abort.
    pub fn can_stop(&self) -> bool {
        matches!(
            self,
            Self::Pull { .. }
                | Self::Merge { .. }
                | Self::Rebase { .. }
                | Self::CherryPick { .. }
                | Self::Revert { .. }
                | Self::GitFlowFinish { .. }
                | Self::Restack { .. }
        )
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

/// Whether a stash list text was made by Glowfly before an operation, or is git's own kept
/// "autostash".
pub fn is_auto_stash(list_text: &str) -> bool {
    split_message(list_text).1.starts_with(AUTO_STASH_PREFIX) || list_text == GIT_AUTOSTASH
}

/// How putting an auto-stash back went.
#[derive(Clone, Debug, PartialEq)]
pub enum PutBack {
    /// The changes are back and the entry is gone.
    Restored,
    /// The changes are back and the entry is gone, but git couldn't restore the staged state,
    /// so everything is unstaged.
    RestoredUnstaged,
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

/// Runs `git stash pop --index stash@{index}` (found by `prepare_put_back` for `oid`) and tells
/// what happened, with the commands and their output for the log. When git refuses to restore
/// the index ("Try without --index": nothing changed, the entry is kept), it runs a plain
/// `git stash pop` straight away and reports `RestoredUnstaged` when that works.
pub fn pop_put_back(workdir: &Path, index: usize, oid: Oid) -> (PutBack, String) {
    let (ok, mut output, listed) = pop(workdir, &args(&["stash", "pop", "--index", &format!("stash@{{{index}}}")]), oid);
    let paths = conflicted_paths(workdir);
    if !ok && listed && paths.is_empty() && output.to_lowercase().contains("without --index") {
        let Some(again) = list(workdir).ok().and_then(|l| l.iter().find(|s| s.oid == oid).map(|s| s.index)) else {
            return (PutBack::Failed, output);
        };
        let (ok, text, listed) = pop(workdir, &args(&["stash", "pop", &format!("stash@{{{again}}}")]), oid);
        output.push('\n');
        output.push_str(&text);
        return match classify(workdir, ok, listed) {
            PutBack::Restored => (PutBack::RestoredUnstaged, output),
            other => (other, output),
        };
    }
    (classify_with(ok, listed, paths), output)
}

/// Puts stash `oid` back with `git stash pop --index` only, never falling back to a plain pop:
/// when git can't restore the staged state too, or anything else goes wrong, the entry stays
/// in the list (`Failed` or `Conflicted`). Refused, changing nothing, while an operation is in
/// progress or files are conflicted (`NotReady`), or when the stash is gone (`Gone`).
pub fn pop_index(workdir: &Path, oid: Oid) -> (PutBack, String) {
    let index = match prepare_put_back(workdir, oid) {
        Ok(i) => i,
        Err(refused) => return refused,
    };
    let (ok, output, listed) = pop(workdir, &args(&["stash", "pop", "--index", &format!("stash@{{{index}}}")]), oid);
    (classify_with(ok, listed, conflicted_paths(workdir)), output)
}

/// Runs one pop command: whether git said it worked, the command with its output, and whether
/// stash `oid` is still listed afterwards.
fn pop(workdir: &Path, cmd: &[String], oid: Oid) -> (bool, String, bool) {
    let (ok, text) = gitcli::run(workdir, cmd);
    let mut output = format!("$ git {}", cmd.join(" "));
    if !text.is_empty() {
        output.push('\n');
        output.push_str(text.trim_end());
    }
    let listed = list(workdir).map(|l| l.iter().any(|s| s.oid == oid)).unwrap_or(true);
    (ok, output, listed)
}

fn classify(workdir: &Path, ok: bool, listed: bool) -> PutBack {
    if ok && !listed { PutBack::Restored } else { classify_with(ok, listed, conflicted_paths(workdir)) }
}

/// A pop that didn't simply work: conflicts when git kept the entry and left conflicted
/// `paths`, a failure otherwise.
fn classify_with(ok: bool, listed: bool, paths: Vec<String>) -> PutBack {
    match (ok, listed) {
        (true, false) => PutBack::Restored,
        (false, true) if !paths.is_empty() => PutBack::Conflicted { paths },
        _ => PutBack::Failed,
    }
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

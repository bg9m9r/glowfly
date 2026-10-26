//! Lost commits: the tips of history that no ref keeps, found by `git fsck` without its
//! reflogs (`--no-reflogs`), so commits only a reflog entry holds (reset away, amended, rebased,
//! on a deleted branch) count as lost too. fsck is read-only, takes no locks and never runs
//! with `--lost-found` (which writes into the repository). The stash list's own entries live in
//! the `refs/stash` reflog, so they are dropped from the result; a dropped stash (a commit with
//! the shape `git stash push` makes) is named as one. The command lines that keep a dropped
//! stash run through gitcli, so hooks and config apply.

use crate::commit_text;
use crate::error::{Error, Result};
use crate::gitcli::{self, args};
use git2::{Commit, Oid, Repository};
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

/// The most lost commits listed (the newest).
pub const MAX_LOST: usize = 500;
/// How long fsck may run before it is stopped.
pub const FSCK_TIMEOUT: Duration = Duration::from_secs(180);

/// What a lost commit is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LostKind {
    Commit,
    /// A dropped stash: `base` is the commit it was made on, `branch` the branch checked out
    /// then (`None`: detached), `untracked` whether it holds untracked files.
    Stash {
        base: Oid,
        branch: Option<String>,
        untracked: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct LostCommit {
    pub oid: Oid,
    pub kind: LostKind,
    /// The message's first line.
    pub summary: String,
    pub message: String,
    pub author: String,
    /// The committer's time, Unix seconds.
    pub time: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LostCommits {
    /// Newest first.
    pub commits: Vec<LostCommit>,
    /// Cut at `MAX_LOST`.
    pub capped: bool,
    /// fsck reported problems, so some may be missing.
    pub incomplete: bool,
}

pub fn fsck_args() -> Vec<String> {
    args(&["fsck", "--connectivity-only", "--no-reflogs", "--no-progress"])
}

/// The commits of fsck's `dangling commit <oid>` lines, in order, each once.
pub fn parse(stdout: &str) -> Vec<Oid> {
    let mut seen = HashSet::new();
    stdout
        .lines()
        .filter_map(|l| l.strip_prefix("dangling commit "))
        .filter_map(|rest| rest.split_whitespace().next())
        .filter_map(|id| Oid::from_str(id).ok())
        .filter(|o| seen.insert(*o))
        .collect()
}

fn message_of(c: &Commit) -> String {
    commit_text::message(c).into_owned()
}

/// A stash's kind when `c` has the shape `git stash push` makes: its first parent the base, its
/// second the index commit on that same base, and an optional third, root commit of untracked
/// files; its message "WIP on <branch>: …" or "On <branch>: …".
fn stash_shape(c: &Commit) -> Option<LostKind> {
    let n = c.parent_count();
    if !(2..=3).contains(&n) {
        return None;
    }
    let message = message_of(c);
    let rest = message.strip_prefix("WIP on ").or_else(|| message.strip_prefix("On "))?;
    let (branch, _) = rest.split_once(": ")?;
    let base = c.parent_id(0).ok()?;
    let index = c.parent(1).ok()?;
    if !message_of(&index).starts_with("index on ") || index.parent_id(0).ok()? != base {
        return None;
    }
    if n == 3 {
        let untracked = c.parent(2).ok()?;
        if untracked.parent_count() != 0 || !message_of(&untracked).starts_with("untracked files on ") {
            return None;
        }
    }
    let branch = (branch != "(no branch)").then(|| branch.to_owned());
    Some(LostKind::Stash { base, branch, untracked: n == 3 })
}

/// The lost commits of the repository at `workdir`, newest first.
pub fn find(workdir: &Path) -> Result<LostCommits> {
    let out = gitcli::output(workdir, &fsck_args(), "", gitcli::NO_PROMPTS, Some(FSCK_TIMEOUT));
    if out.timed_out {
        return Err(Error::Other("git fsck took too long".into()));
    }
    let oids = parse(&out.stdout);
    let failed = out.stderr.lines().any(|l| !l.trim().is_empty() && !l.trim_start().starts_with("notice:"));
    if !out.ok && oids.is_empty() && failed {
        return Err(Error::Other(out.stderr.trim().to_owned()));
    }
    let repo = Repository::open(workdir)?;
    let kept: HashSet<Oid> = match repo.reflog("refs/stash") {
        Ok(log) => log.iter().map(|e| e.id_new()).collect(),
        Err(_) => HashSet::new(),
    };
    let mut commits: Vec<LostCommit> = oids
        .into_iter()
        .filter(|o| !kept.contains(o))
        .filter_map(|o| repo.find_commit(o).ok())
        .map(|c| {
            let message = message_of(&c);
            LostCommit {
                oid: c.id(),
                kind: stash_shape(&c).unwrap_or(LostKind::Commit),
                summary: message.lines().next().unwrap_or_default().to_owned(),
                author: commit_text::author(&c).name,
                time: c.committer().when().seconds(),
                message,
            }
        })
        .collect();
    commits.sort_by(|a, b| b.time.cmp(&a.time).then_with(|| a.oid.cmp(&b.oid)));
    let capped = commits.len() > MAX_LOST;
    commits.truncate(MAX_LOST);
    Ok(LostCommits { commits, capped, incomplete: !out.ok })
}

/// Applies the stash-like commit `oid` to the working tree.
pub fn apply_stash(oid: Oid) -> Vec<String> {
    args(&["stash", "apply", &oid.to_string()])
}

/// Puts the stash-like commit `oid` back at the top of the stash list, named `message`.
pub fn store_stash(oid: Oid, message: &str) -> Vec<String> {
    args(&["stash", "store", "-m", message, &oid.to_string()])
}

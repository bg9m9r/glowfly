//! Removing, locking, unlocking and pruning worktrees: the command lines, reading git's refusal
//! to remove one, and `remove`, which checks for a lock and for changes before git runs and then
//! judges the result by listing the worktrees again.

use crate::error::Result;
use crate::gitcli;
use git2::{Repository, Status, StatusOptions};
use std::path::Path;

/// How hard `git worktree remove` is pushed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RemoveForce {
    None,
    /// `--force`: removes a worktree with changed or untracked files.
    Changes,
    /// `--force --force`: also removes a locked worktree (and covers changes too).
    Lock,
}

/// `git worktree remove` for `path`, with the flags `force` needs.
pub fn remove_cmd(path: &Path, force: RemoveForce) -> Vec<String> {
    let mut cmd = vec!["worktree".to_owned(), "remove".to_owned()];
    let n = match force {
        RemoveForce::None => 0,
        RemoveForce::Changes => 1,
        RemoveForce::Lock => 2,
    };
    cmd.extend(std::iter::repeat_n("--force".to_owned(), n));
    cmd.push(path.display().to_string());
    cmd
}

/// `git worktree lock` for `path`; a reason that is blank after trimming sends no `--reason`,
/// and line breaks in it become spaces.
pub fn lock_cmd(path: &Path, reason: Option<&str>) -> Vec<String> {
    let mut cmd = vec!["worktree".to_owned(), "lock".to_owned()];
    let reason = reason.map(|r| r.replace(['\r', '\n'], " ")).filter(|r| !r.trim().is_empty());
    if let Some(r) = reason {
        cmd.extend(["--reason".to_owned(), r.trim().to_owned()]);
    }
    cmd.push(path.display().to_string());
    cmd
}

/// `git worktree unlock` for `path`.
pub fn unlock_cmd(path: &Path) -> Vec<String> {
    vec!["worktree".to_owned(), "unlock".to_owned(), path.display().to_string()]
}

/// `git worktree prune --verbose`: drops the records of missing, unlocked worktrees.
pub fn prune_cmd() -> Vec<String> {
    vec!["worktree".to_owned(), "prune".to_owned(), "--verbose".to_owned()]
}

/// Why a worktree wasn't removed without force.
#[derive(Clone, Debug, PartialEq)]
pub enum Refusal {
    /// It has this many changed or untracked files (0 when only git's message said so).
    Changes(usize),
    /// It is locked, with this reason ("" without one).
    Locked(String),
}

impl Refusal {
    /// The force that gets past this refusal.
    pub fn force(&self) -> RemoveForce {
        match self {
            Refusal::Changes(_) => RemoveForce::Changes,
            Refusal::Locked(_) => RemoveForce::Lock,
        }
    }
}

/// Reads git's refusal to remove a worktree from its output. The checks look for the flags git
/// suggests rather than its English words, since the CLI runs in the user's locale.
pub fn refusal_in(output: &str) -> Option<Refusal> {
    if output.contains("-f -f") || output.contains("locked working tree") {
        let reason = output.lines().find_map(|l| l.split_once("lock reason: ").map(|(_, r)| r.trim().to_owned())).unwrap_or_default();
        return Some(Refusal::Locked(reason));
    }
    (output.contains("--force") || output.contains("submodules")).then_some(Refusal::Changes(0))
}

/// How a remove ended.
#[derive(Clone, Debug, PartialEq)]
pub enum RemoveOutcome {
    /// The record and the folder are gone.
    Removed,
    /// git dropped the record, but the folder (or part of it) is still on disk.
    FolderLeft,
    /// Nothing changed: it needs force.
    NeedsForce(Refusal),
    /// Nothing changed, for another reason (in the output).
    Failed,
}

/// The outcome of `remove` and its output: "$ git …" lines and git's text.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoveReport {
    pub outcome: RemoveOutcome,
    pub output: String,
}

/// The outcome from what is on disk afterwards. git can exit with an error and still drop the
/// record when it can't delete every file, so the listing decides, not the exit code.
pub fn classify(still_registered: bool, folder_exists: bool, output: &str) -> RemoveOutcome {
    if still_registered {
        refusal_in(output).map(RemoveOutcome::NeedsForce).unwrap_or(RemoveOutcome::Failed)
    } else if folder_exists {
        RemoveOutcome::FolderLeft
    } else {
        RemoveOutcome::Removed
    }
}

/// The number of changed, staged or untracked (not ignored) files in the worktree at `worktree`,
/// which is what makes git refuse to remove it without force.
pub fn dirty_count(worktree: &Path) -> Result<usize> {
    let repo = Repository::open(worktree)?;
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(false).include_ignored(false).exclude_submodules(false);
    let statuses = repo.statuses(Some(&mut opts))?;
    Ok(statuses.iter().filter(|e| !e.status().is_empty() && !e.status().intersects(Status::IGNORED)).count())
}

/// Removes the worktree at `path` from the repository `cwd` belongs to. A lock (unless `force`
/// is `Lock`) or changed files (unless forced at all) are reported before git runs; then
/// `git worktree remove` runs through the CLI and the worktrees are listed again.
pub fn remove(cwd: &Path, path: &Path, force: RemoveForce) -> RemoveReport {
    let listed = |p: &Path| crate::repo::worktrees_at(cwd).ok().and_then(|l| l.into_iter().find(|w| crate::paths::same_folder(&w.path, p)));
    let Some(wt) = listed(path) else {
        return RemoveReport {
            outcome: RemoveOutcome::Failed,
            output: format!("{} is no longer a worktree of this repository.", path.display()),
        };
    };
    if let Some(reason) = &wt.locked
        && force != RemoveForce::Lock
    {
        return RemoveReport { outcome: RemoveOutcome::NeedsForce(Refusal::Locked(reason.clone())), output: String::new() };
    }
    if force == RemoveForce::None
        && !wt.missing
        && wt.path.exists()
        && let Ok(n) = dirty_count(&wt.path)
        && n > 0
    {
        return RemoveReport { outcome: RemoveOutcome::NeedsForce(Refusal::Changes(n)), output: String::new() };
    }
    let cmd = remove_cmd(&wt.path, force);
    let (_, text) = gitcli::run(cwd, &cmd);
    let mut output = format!("$ git {}", cmd.join(" "));
    if !text.trim().is_empty() {
        output.push('\n');
        output.push_str(text.trim_end());
    }
    // git's own words only: the echoed command line holds `--force` itself.
    let outcome = classify(listed(&wt.path).is_some(), wt.path.exists(), &text);
    RemoveReport { outcome, output }
}

#[cfg(test)]
#[path = "manage_tests.rs"]
mod tests;

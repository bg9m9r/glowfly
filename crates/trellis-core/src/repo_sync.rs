//! Fetching or pulling a listed repository that may not be open in Trellis (fetch or pull
//! across many repositories). Git never prompts here: a repository that needs a password the
//! credential helper doesn't have fails, and one whose own config or hooks would make git run
//! programs is skipped unless the user trusts it.

use crate::gitcli;
use git2::Repository;
use std::path::Path;
use std::time::Duration;

/// What to do in each repository.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncOp {
    /// `git fetch --all`, with `--prune` when asked.
    Fetch { prune: bool },
    /// `git pull --ff-only`: only a fast-forward of the checked-out branch.
    Pull,
}

/// How one repository's fetch or pull went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It ran and `changed` references moved.
    Updated { changed: usize },
    /// It ran and nothing changed.
    UpToDate,
    /// The repository has no remotes: there is nothing to fetch or pull.
    NoRemotes,
    /// Pull only: the checked-out branch has no upstream (or no branch is checked out).
    NoUpstream,
    /// Skipped: its config or hooks would make git run programs and the user hasn't trusted it.
    Untrusted,
    /// The folder is gone or isn't a repository, or git failed (the reason).
    Failed(String),
}

/// How long one repository's fetch or pull may take before git is stopped.
pub const TIMEOUT: Duration = Duration::from_secs(300);

/// The git command `op` runs.
pub fn command(op: SyncOp) -> Vec<String> {
    match op {
        SyncOp::Fetch { prune: true } => gitcli::args(&["fetch", "--all", "--prune"]),
        SyncOp::Fetch { prune: false } => gitcli::args(&["fetch", "--all"]),
        SyncOp::Pull => gitcli::args(&["pull", "--ff-only"]),
    }
}

/// Fetches or pulls the repository at `path`. Unless `trusted`, its config and hooks are
/// scanned first and it is skipped when they would make git run programs.
pub fn sync(path: &Path, op: SyncOp, trusted: bool) -> Outcome {
    if !path.is_dir() {
        return Outcome::Failed("The folder can't be found.".into());
    }
    let repo = match Repository::open(path) {
        Ok(r) => r,
        Err(e) => return Outcome::Failed(crate::safe_directory::open_error(path, &e).to_string()),
    };
    if !trusted && !crate::trust::scan(&repo).is_empty() {
        return Outcome::Untrusted;
    }
    if repo.remotes().map(|r| r.is_empty()).unwrap_or(true) {
        return Outcome::NoRemotes;
    }
    if op == SyncOp::Pull && !has_upstream(&repo) {
        return Outcome::NoUpstream;
    }
    let before = ref_targets(&repo);
    let out = gitcli::output(path, &command(op), "", gitcli::NO_PROMPTS, Some(TIMEOUT));
    if !out.ok {
        let why = if out.timed_out { format!("git stopped after {} minutes.", TIMEOUT.as_secs() / 60) } else { last_line(&out.combined()) };
        return Outcome::Failed(why);
    }
    // Reopened: libgit2 caches packed references.
    let after = Repository::open(path).map(|r| ref_targets(&r)).unwrap_or_default();
    let changed =
        after.iter().filter(|r| !before.contains(r)).count() + before.iter().filter(|r| !after.iter().any(|a| a.0 == r.0)).count();
    if changed == 0 { Outcome::UpToDate } else { Outcome::Updated { changed } }
}

/// Whether the checked-out branch has an upstream.
fn has_upstream(repo: &Repository) -> bool {
    let Ok(head) = repo.head() else { return false };
    if !head.is_branch() {
        return false;
    }
    let Some(name) = head.shorthand() else { return false };
    repo.find_branch(name, git2::BranchType::Local).and_then(|b| b.upstream()).is_ok()
}

/// Every branch, tag and remote branch with the commit it points at, and the commit HEAD is on.
/// Symbolic references (a remote's HEAD, which a fetch may add) are left out: they move nothing.
fn ref_targets(repo: &Repository) -> Vec<(String, Option<git2::Oid>)> {
    let mut out: Vec<(String, Option<git2::Oid>)> = Vec::new();
    out.push(("HEAD".into(), repo.head().ok().and_then(|h| h.target())));
    if let Ok(refs) = repo.references() {
        for r in refs.flatten().filter(|r| r.kind() == Some(git2::ReferenceType::Direct)) {
            out.push((String::from_utf8_lossy(r.name_bytes()).into_owned(), r.target()));
        }
    }
    out
}

/// The last line of git's output that says something (the error, after progress lines).
fn last_line(text: &str) -> String {
    let line = text.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("git failed.");
    crate::text::redact_credentials(line)
}

#[cfg(test)]
mod tests;

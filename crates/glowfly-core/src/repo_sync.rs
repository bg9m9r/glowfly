//! Fetching or pulling a listed repository that may not be open in Glowfly (fetch or pull
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
    /// The folder is gone or isn't a repository, or git failed: `reason` is one line, `detail`
    /// git's whole output (redacted and capped), or empty when there is nothing beyond the reason.
    Failed { reason: String, detail: String },
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

/// Fetches or pulls the repository at `path`, whose saved decision is `trust`. Its config and
/// hooks are scanned first and it is skipped unless the decision allows what they run
/// (`trust::allows`: a Trusted one whose print covers them, or none when nothing runs).
pub fn sync(path: &Path, op: SyncOp, trust: Option<&crate::settings::SavedTrust>) -> Outcome {
    if !path.is_dir() {
        return Outcome::Failed { reason: "The folder can't be found.".into(), detail: String::new() };
    }
    let repo = match Repository::open(path) {
        Ok(r) => r,
        Err(e) => return Outcome::Failed { reason: crate::safe_directory::open_error(path, &e).to_string(), detail: String::new() },
    };
    if !crate::trust::allows(trust, &crate::trust::scan(&repo)) {
        return Outcome::Untrusted;
    }
    if repo.remotes().map(|r| r.is_empty()).unwrap_or(true) {
        return Outcome::NoRemotes;
    }
    if op == SyncOp::Pull && !has_upstream(&repo) {
        return Outcome::NoUpstream;
    }
    let before = ref_targets(&repo);
    // Hooks that follow the working tree come with the commits a pull merges: the pull is
    // pinned to commits whose hooks the decision covers (trust/pull.rs), else skipped.
    let follows = crate::trust::scan_now(&repo).follows;
    if op == SyncOp::Pull && !follows.is_empty() {
        let pull = crate::trust::pull::Pull::of(&command(op)).expect("a pull");
        let mut timed_out = false;
        let mut git = |args: &[String], input: &str| {
            let out = gitcli::output(path, args, input, gitcli::NO_PROMPTS, Some(TIMEOUT));
            timed_out |= out.timed_out;
            (out.ok, out.combined())
        };
        return match crate::trust::pull::run_pinned(path, &pull, &follows, trust, None, &mut git) {
            crate::trust::pull::PullRun::Refused { .. } => Outcome::Untrusted,
            crate::trust::pull::PullRun::Ran { ok: false, output } if timed_out => {
                Outcome::Failed { reason: format!("git stopped after {} minutes.", TIMEOUT.as_secs() / 60), detail: error_detail(&output) }
            }
            crate::trust::pull::PullRun::Ran { ok: false, output } => failure(&output),
            crate::trust::pull::PullRun::Ran { ok: true, .. } => changed(path, &before),
        };
    }
    let out = gitcli::output(path, &command(op), "", gitcli::NO_PROMPTS, Some(TIMEOUT));
    if !out.ok {
        let text = out.combined();
        if out.timed_out {
            let reason = format!("git stopped after {} minutes.", TIMEOUT.as_secs() / 60);
            return Outcome::Failed { reason, detail: error_detail(&text) };
        }
        return failure(&text);
    }
    changed(path, &before)
}

/// Up to date, or how many references moved since `before`.
fn changed(path: &Path, before: &[(String, Option<git2::Oid>)]) -> Outcome {
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

/// The most lines of git's output a failure keeps, and the most characters.
const DETAIL_LINES: usize = 40;
const DETAIL_CHARS: usize = 4000;

/// A failure from git's output: its last line as the reason, the whole output as the detail.
fn failure(text: &str) -> Outcome {
    Outcome::Failed { reason: last_line(text), detail: error_detail(text) }
}

/// What a line of git's output finally shows: progress rewrites the line after each `\r`.
fn shown(line: &str) -> &str {
    line.rsplit('\r').next().unwrap_or(line).trim()
}

/// The last line of git's output that says something (the error, after progress lines).
fn last_line(text: &str) -> String {
    let line = text.lines().map(shown).rfind(|l| !l.is_empty()).unwrap_or("git failed.");
    crate::text::redact_credentials(line)
}

/// git's output for a failed run's hover: each line as it finally showed, credentials in URLs
/// redacted, empty lines dropped, the last 40 lines and at most 4000 characters (the end kept).
pub(crate) fn error_detail(text: &str) -> String {
    let lines: Vec<String> = text.lines().map(shown).filter(|l| !l.is_empty()).map(crate::text::redact_credentials).collect();
    let out = lines[lines.len().saturating_sub(DETAIL_LINES)..].join("\n");
    let n = out.chars().count();
    if n <= DETAIL_CHARS {
        return out;
    }
    let tail: String = out.chars().skip(n - (DETAIL_CHARS - 1)).collect();
    format!("…{tail}")
}

#[cfg(test)]
mod tests;

//! Applying a series of emailed patches as commits with `git am`: the command lines, where a
//! stopped series is (read from `rebase-apply/`), and what git said when it stopped.

use crate::gitcli::args;
use git2::{Repository, RepositoryState};
use std::path::Path;

/// The command that applies the series in `patch` as commits on HEAD, with a three-way
/// fallback for a patch that doesn't apply when `three_way`.
pub fn am_cmd(patch: &str, three_way: bool) -> Vec<String> {
    args(&["am", if three_way { "--3way" } else { "--no-3way" }, patch])
}

/// Commit the stopped patch (as staged) and apply the rest.
pub const AM_CONTINUE: &[&str] = &["am", "--continue"];
/// Leave the stopped patch out and apply the rest.
pub const AM_SKIP: &[&str] = &["am", "--skip"];
/// Stop applying and put the branch back where it was.
pub const AM_ABORT: &[&str] = &["am", "--abort"];

/// Where a stopped `git am` is.
#[derive(Clone, Debug, PartialEq)]
pub struct AmProgress {
    /// The patch it stopped at (1-based).
    pub step: usize,
    /// Patches in the series.
    pub total: usize,
    /// The stopped patch's subject; empty when unknown.
    pub subject: String,
    /// Whether the index has conflicts.
    pub conflicted: bool,
}

/// Where the `git am` in progress in `repo` stopped; None unless one is. Never fails: a file
/// it can't read gives 0 or an empty subject.
pub(crate) fn progress_of(repo: &Repository) -> Option<AmProgress> {
    if repo.state() != RepositoryState::ApplyMailbox {
        return None;
    }
    let dir = repo.path().join("rebase-apply");
    let number = |name: &str| std::fs::read_to_string(dir.join(name)).ok().and_then(|s| s.trim().parse::<usize>().ok()).unwrap_or(0);
    let (next, last) = (number("next"), number("last"));
    Some(AmProgress {
        step: next.min(last),
        total: last,
        subject: current_subject(repo.path()).unwrap_or_default(),
        conflicted: repo.index().is_ok_and(|i| i.has_conflicts()),
    })
}

/// The subject of the patch `git am` stopped at, from the repository at `git_dir`: its
/// "Subject: " line in `rebase-apply/info`, else the first line of its message.
pub(crate) fn current_subject(git_dir: &Path) -> Option<String> {
    let dir = git_dir.join("rebase-apply");
    let first = |text: String, header: bool| {
        text.lines()
            .filter_map(|l| if header { l.strip_prefix("Subject: ") } else { Some(l) })
            .map(str::trim)
            .find(|l| !l.is_empty())
            .map(str::to_owned)
    };
    let info = std::fs::read_to_string(dir.join("info")).ok().and_then(|t| first(t, true));
    info.or_else(|| std::fs::read_to_string(dir.join("msg")).ok().and_then(|t| first(t, false)))
}

/// What `git am`'s output says about where and why it stopped.
#[derive(Debug, Default, PartialEq)]
pub struct AmStop {
    /// The patch it stopped at (its number in the series) and subject.
    pub at: Option<(u32, String)>,
    /// A three-way fallback left conflicts.
    pub conflicts: bool,
    /// The first problem `git apply` reported (see [`super::failure_summary`]).
    pub problem: Option<String>,
    /// The file isn't a series of emails.
    pub not_mail: bool,
}

/// Reads `git am`'s `output` (see [`AmStop`]).
pub fn am_stop(output: &str) -> AmStop {
    let at = output.lines().find_map(|l| {
        let rest = l.trim_end().strip_prefix("Patch failed at ")?;
        let (n, subject) = rest.split_once(' ').unwrap_or((rest, ""));
        Some((n.parse().ok()?, subject.to_owned()))
    });
    AmStop {
        at,
        conflicts: output.lines().any(|l| l.starts_with("CONFLICT (")),
        problem: super::failure_summary(output),
        not_mail: output.lines().any(|l| l.trim_end() == "Patch format detection failed."),
    }
}

#[cfg(test)]
#[path = "am_tests.rs"]
mod tests;

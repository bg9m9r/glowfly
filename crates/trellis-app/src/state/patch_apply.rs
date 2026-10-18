//! The Apply patch dialog (`Modal::ApplyPatch`): what the file holds, how to apply it (its
//! changes with `git apply` options, or as commits with `git am`), why it can't be applied now,
//! and the result texts of both jobs and of the in-progress series' buttons. Pure.

use super::display_name;
use crate::patch::{ApplyHow, ApplyRequest};
use std::path::PathBuf;
use trellis_core::patch_ops::{self, ApplyOptions, OnFail, PatchSummary};
use trellis_core::repo::Snapshot;

/// The summary line of a file with nothing to apply (also why Apply is disabled).
pub const NOTHING_TO_APPLY: &str = "This file holds no changes git can apply.";
/// Why Apply as commits is unavailable for a plain diff.
pub const NO_MESSAGES: &str = "This file has no commit messages, so only its changes can be applied.";
pub const STAGED_FIRST: &str = "Commit or unstage your staged changes first: applying as commits needs nothing staged.";
pub const RESOLVE_FIRST: &str = "Resolve the conflicted files first.";
/// Why Working tree only is unavailable with the three-way fallback.
pub const THREE_WAY_INDEX: &str = "The three-way fallback works through the index.";

/// How the dialog applies the patch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ApplyMode {
    /// Its changes, to the working tree (and maybe the index).
    Changes,
    /// As commits on HEAD.
    Commits,
}

/// The Apply patch dialog's form.
#[derive(Clone, Debug, PartialEq)]
pub struct ApplyForm {
    /// The repository it was opened for.
    pub workdir: PathBuf,
    pub path: PathBuf,
    /// The file's own name.
    pub name: String,
    pub summary: PatchSummary,
    pub mode: ApplyMode,
    pub opts: ApplyOptions,
    /// Apply as commits with a three-way fallback.
    pub am_three_way: bool,
}

impl ApplyForm {
    /// The dialog for the file at `path`: as commits when it holds emailed commits, else its
    /// changes to the working tree only, changing nothing when part doesn't apply.
    pub fn new(workdir: PathBuf, path: PathBuf, summary: PatchSummary) -> Self {
        let mode = if summary.commits > 0 { ApplyMode::Commits } else { ApplyMode::Changes };
        Self { workdir, name: display_name(&path), path, summary, mode, opts: ApplyOptions::default(), am_three_way: true }
    }

    /// What Apply sends. The three-way fallback always goes through the index.
    pub fn request(&self) -> ApplyRequest {
        let how = match self.mode {
            ApplyMode::Changes => {
                let mut o = self.opts;
                o.index |= o.on_fail == OnFail::ThreeWay;
                ApplyHow::Changes(o)
            }
            ApplyMode::Commits => ApplyHow::Commits { three_way: self.am_three_way },
        };
        ApplyRequest { workdir: self.workdir.clone(), path: self.path.clone(), how, commits: self.summary.commits }
    }

    /// "3 commits · 5 files" or "Changes to 2 files"; None when it touches no file.
    pub fn summary_line(&self) -> Option<String> {
        let files = match self.summary.files.len() {
            0 => return None,
            n => count(n, "file"),
        };
        Some(match self.summary.commits {
            0 => format!("Changes to {files}"),
            c => format!("{} · {files}", count(c, "commit")),
        })
    }
}

/// "1 file", "3 files".
fn count(n: usize, noun: &str) -> String {
    if n == 1 { format!("1 {noun}") } else { format!("{n} {noun}s") }
}

fn conflicted(snap: &Snapshot) -> bool {
    snap.unstaged.iter().any(|f| f.status == '!')
}

/// Why `git am` can't start now: it needs an empty index, no operation in progress and no
/// conflicts.
pub fn am_blocked(snap: &Snapshot) -> Option<String> {
    if !snap.staged.is_empty() {
        return Some(STAGED_FIRST.to_owned());
    }
    if let Some(op) = trellis_core::rebase::operation_noun(snap.state) {
        return Some(format!("Finish or abort the {op} in progress first."));
    }
    conflicted(snap).then(|| RESOLVE_FIRST.to_owned())
}

/// Why applying to the index can't start now.
pub fn index_blocked(snap: &Snapshot) -> Option<&'static str> {
    conflicted(snap).then_some(RESOLVE_FIRST)
}

/// Why the dialog's choice can't be applied now.
pub fn apply_blocked(form: &ApplyForm, snap: &Snapshot) -> Option<String> {
    if form.summary.files.is_empty() {
        return Some(NOTHING_TO_APPLY.to_owned());
    }
    request_blocked(&form.request(), snap)
}

/// Why `req` can't run now (checked again by `update`).
pub fn request_blocked(req: &ApplyRequest, snap: &Snapshot) -> Option<String> {
    match req.how {
        ApplyHow::Commits { .. } if req.commits == 0 => Some(NO_MESSAGES.to_owned()),
        ApplyHow::Commits { .. } => am_blocked(snap),
        ApplyHow::Changes(o) if o.index => index_blocked(snap).map(str::to_owned),
        ApplyHow::Changes(_) => None,
    }
}

/// The last line of a job's output that isn't a command echo.
fn last_line(output: &str) -> &str {
    output.lines().rfind(|l| !l.starts_with("$ ") && !l.trim().is_empty()).unwrap_or("")
}

/// Names, at most three, then "and N more".
fn listed(names: &[String]) -> String {
    let mut out = names.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
    if names.len() > 3 {
        out.push_str(&format!(" and {} more", names.len() - 3));
    }
    out
}

/// The headline after applying `name`'s changes as `o` says.
pub fn changes_done(name: &str, o: &ApplyOptions) -> String {
    let index = o.index || o.on_fail == OnFail::ThreeWay;
    match (o.reverse, index) {
        (false, false) => format!("Applied {name} to the working tree."),
        (false, true) => format!("Applied {name} and staged its changes."),
        (true, false) => format!("Took {name}'s changes out of the working tree."),
        (true, true) => format!("Took {name}'s changes out of the working tree and the index."),
    }
}

/// The headline after applying `name`'s changes failed: conflicts a three-way fallback left,
/// hunks saved to `.rej` files, else why nothing was changed.
pub fn changes_failed(name: &str, o: &ApplyOptions, output: &str) -> String {
    match o.on_fail {
        OnFail::ThreeWay => {
            let files = patch_ops::three_way_conflicts(output);
            if !files.is_empty() {
                return format!("Applied {name} with conflicts in {}. Resolve them in the conflict view.", count(files.len(), "file"));
            }
        }
        OnFail::Reject => {
            let rejects = patch_ops::rejects(output);
            if !rejects.is_empty() {
                let hunks: u32 = rejects.iter().map(|(_, n)| n).sum();
                let names: Vec<String> = rejects.iter().map(|(f, _)| patch_ops::rej_name(f)).collect();
                let fit = if hunks == 1 { "1 hunk didn't fit and was".to_owned() } else { format!("{hunks} hunks didn't fit and were") };
                return format!("Applied part of {name}: {fit} saved to {}.", listed(&names));
            }
        }
        OnFail::Refuse => {}
    }
    let why = patch_ops::failure_summary(output).unwrap_or_else(|| last_line(output).to_owned());
    format!("{name} doesn't apply: {}. Nothing was changed.", why.trim_end_matches('.'))
}

/// The headline after applying `name` as `commits` commits.
pub fn am_done(name: &str, commits: usize) -> String {
    if commits == 1 { format!("Applied {name} as a commit.") } else { format!("Applied {name} as {commits} commits.") }
}

/// The headline after `git am` stopped: on `name`, or (None) on the series in progress.
pub fn am_failed(name: Option<&str>, output: &str) -> String {
    let stop = patch_ops::am_stop(output);
    if let (true, Some(n)) = (stop.not_mail, name) {
        return format!("{n} has no commit messages, so it can't be applied as commits. Apply its changes instead.");
    }
    let lead = match name {
        Some(n) => format!("Applying {n} stopped"),
        None => "Applying patches stopped".to_owned(),
    };
    let at = stop.at.as_ref().map(|(n, s)| format!(" at patch {n} ({s})")).unwrap_or_default();
    if stop.conflicts {
        return format!("{lead}{at} with conflicts. Resolve them, then Continue; or Skip patch or Abort.");
    }
    let why = stop.problem.unwrap_or_else(|| last_line(output).to_owned());
    if stop.at.is_none() && why.is_empty() {
        return format!("{lead}.");
    }
    format!("{lead}{at}: {}. Make the change by hand and Continue; or Skip patch or Abort.", why.trim_end_matches('.'))
}

/// The headline after the in-progress series' button `label` worked.
pub fn am_resume_done(label: &str) -> String {
    if label == super::super::am_banner::AM_ABORT {
        "Stopped applying patches. The branch is back where it was.".to_owned()
    } else {
        "Applied the remaining patches.".to_owned()
    }
}

#[cfg(test)]
#[path = "patch_apply_tests.rs"]
mod tests;

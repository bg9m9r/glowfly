//! Comparing two versions of a line of commits (before and after a rebase, amend, reset or
//! force push) commit by commit with `git range-diff`. The two ends are given, or read from a
//! ref's reflog (`bases::last_rewrite`); each side's range is cut at a mainline or the two
//! versions' merge base (`bases::ranges`); git's output is read by `parse.rs`. It only reads:
//! no ref, index or working file changes. git runs through gitcli without prompts and with a
//! timeout; refs, ids and summaries are read with libgit2.

pub mod bases;
pub mod parse;

use crate::error::{Error, Result};
use crate::gitcli::{self, GitOutput};
use crate::repo::{FileDiff, LineCount};
use crate::text::printable;
use git2::{Oid, Repository};
use std::path::Path;
use std::time::Duration;

/// How long one comparison may take before git is stopped.
pub const COMPARE_TIMEOUT: Duration = Duration::from_secs(120);

/// The most commits (merges left out) either side may have.
pub const MAX_COMMITS: usize = 300;

/// One side's commits: those `tip` has and `base` lacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub base: Oid,
    pub tip: Oid,
}

/// The two versions to compare.
#[derive(Clone, Debug, PartialEq)]
pub enum Ends {
    /// Two given tips, old first.
    Tips { old: Oid, new: Oid },
    /// A ref (`refs/heads/x` or `refs/remotes/r/x`) before its last rewrite, and now.
    LastRewrite { refname: String },
}

/// How a commit fared between the two versions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Both versions make the same change.
    Unchanged,
    /// Both have it, with differences (the pair has an interdiff).
    Changed,
    /// Only the old version has it: dropped, squashed or changed past recognition.
    Removed,
    /// Only the new version has it.
    Added,
}

/// One version of a commit.
#[derive(Clone, Debug, PartialEq)]
pub struct Side {
    /// Its place in its range, 1-based as git numbers it.
    pub index: u32,
    pub oid: Oid,
    pub summary: String,
}

/// One commit's two versions.
#[derive(Clone, Debug, PartialEq)]
pub struct Pair {
    pub status: Status,
    pub old: Option<Side>,
    pub new: Option<Side>,
    /// The diff between the two versions' patches, for a changed pair.
    pub interdiff: Option<FileDiff>,
}

impl Pair {
    /// The new version's summary, else the old one's.
    pub fn summary(&self) -> &str {
        self.new.as_ref().or(self.old.as_ref()).map_or("", |s| s.summary.as_str())
    }

    /// The interdiff's outer '+' and '-' lines; zero without one.
    pub fn counts(&self) -> LineCount {
        let mut c = LineCount::default();
        for l in self.interdiff.iter().flat_map(|d| &d.hunks).flat_map(|h| &h.lines) {
            match l.origin {
                '+' => c.added += 1,
                '-' => c.removed += 1,
                _ => {}
            }
        }
        c
    }
}

/// The reflog entry a `LastRewrite` comparison starts from.
#[derive(Clone, Debug, PartialEq)]
pub struct Rewrite {
    /// The ref's tip before it.
    pub before: Oid,
    /// Unix seconds.
    pub at: i64,
    /// git's reflog message.
    pub message: String,
}

/// A finished comparison.
#[derive(Clone, Debug, PartialEq)]
pub struct Compared {
    pub old: Range,
    pub new: Range,
    /// The branch that cut the ranges; None when the merge base did.
    pub mainline: Option<String>,
    pub rewrite: Option<Rewrite>,
    /// In git's order.
    pub pairs: Vec<Pair>,
}

/// What a comparison gave: the pairs, or why there is nothing to compare.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Compared(Compared),
    /// The ref's reflog shows no rewrite.
    NoRewrite {
        refname: String,
    },
    NoSharedHistory,
    /// A side has more than `MAX_COMMITS` commits (each count is capped at one more).
    TooMany {
        old: usize,
        new: usize,
    },
}

/// `git range-diff --no-color --abbrev=40 <old.base>..<old.tip> <new.base>..<new.tip>`.
pub fn command(old: Range, new: Range) -> Vec<String> {
    let (a, b) = (format!("{}..{}", old.base, old.tip), format!("{}..{}", new.base, new.tip));
    gitcli::args(&["range-diff", "--no-color", "--abbrev=40", &a, &b])
}

/// Compares the two versions `ends` names in the repository at `workdir`, cutting the ranges
/// at `mainline` (name and tip) when it has one.
pub fn compare(workdir: &Path, ends: &Ends, mainline: Option<(String, Oid)>) -> Result<Outcome> {
    compare_capped(workdir, ends, mainline, MAX_COMMITS)
}

pub(crate) fn compare_capped(workdir: &Path, ends: &Ends, mainline: Option<(String, Oid)>, cap: usize) -> Result<Outcome> {
    let repo = Repository::open(workdir)?;
    let (old, new, rewrite) = match ends {
        Ends::Tips { old, new } => (*old, *new, None),
        Ends::LastRewrite { refname } => {
            let Some(r) = bases::last_rewrite(&repo, refname)? else {
                return Ok(Outcome::NoRewrite { refname: refname.clone() });
            };
            (r.before, repo.refname_to_id(refname)?, Some(r))
        }
    };
    let (old_r, new_r, cut) = match bases::ranges(&repo, old, new, mainline.as_ref().map(|m| m.1))? {
        bases::Cut::Ranges(a, b, cut) => (a, b, cut),
        bases::Cut::NoShared => return Ok(Outcome::NoSharedHistory),
    };
    let (n_old, n_new) = (count(&repo, old_r, cap)?, count(&repo, new_r, cap)?);
    if n_old > cap || n_new > cap {
        return Ok(Outcome::TooMany { old: n_old, new: n_new });
    }
    let out = gitcli::output(workdir, &command(old_r, new_r), "", gitcli::NO_PROMPTS, Some(COMPARE_TIMEOUT));
    read_output(&out)?;
    let mut pairs = Vec::new();
    for raw in parse::pairs(&out.stdout)? {
        let old = raw.old.map(|s| side(&repo, s)).transpose()?;
        let new = raw.new.map(|s| side(&repo, s)).transpose()?;
        pairs.push(Pair { status: raw.status, old, new, interdiff: raw.interdiff });
    }
    let mainline = mainline.filter(|_| cut).map(|m| m.0);
    Ok(Outcome::Compared(Compared { old: old_r, new: new_r, mainline, rewrite, pairs }))
}

/// The commits `r` holds (merges left out, as range-diff does), up to `cap + 1`.
fn count(repo: &Repository, r: Range, cap: usize) -> Result<usize> {
    let mut walk = repo.revwalk()?;
    walk.push(r.tip)?;
    walk.hide(r.base)?;
    let mut n = 0;
    for oid in walk {
        if repo.find_commit(oid?)?.parent_count() <= 1 {
            n += 1;
        }
        if n > cap {
            break;
        }
    }
    Ok(n)
}

/// A side git printed, its id resolved and its summary read with libgit2.
fn side(repo: &Repository, (index, hex): (u32, String)) -> Result<Side> {
    let commit = if hex.len() == 40 { repo.find_commit(Oid::from_str(&hex)?)? } else { repo.find_commit_by_prefix(&hex)? };
    Ok(Side { index, oid: commit.id(), summary: commit.summary().unwrap_or_default().to_owned() })
}

/// Whether git's run worked: a timeout, a git without range-diff (Unsupported) or git's own
/// error are errors.
pub fn read_output(out: &GitOutput) -> Result<()> {
    if out.timed_out {
        return Err(Error::Other("Comparing took too long, and git was stopped.".into()));
    }
    if out.code == Some(0) {
        return Ok(());
    }
    let missing = out.stderr.contains("'range-diff' is not a git command");
    if missing || (out.code == Some(129) && out.stderr.contains("usage: git range-diff")) {
        return Err(Error::Unsupported(crate::git_version::Feature::RangeDiff.needs_text(None)));
    }
    let text = printable(out.stderr.trim());
    Err(Error::Other(if text.is_empty() { "git range-diff failed".into() } else { text }))
}

#[cfg(test)]
mod tests;

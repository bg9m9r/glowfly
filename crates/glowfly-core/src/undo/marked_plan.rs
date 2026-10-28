//! Checking a conflict change (`marked.rs`) before it runs and before its undo or redo: the
//! paths can be named safely (no two that fold to one name where case doesn't count, no
//! symbolic link on the way), what it would write over can be put back, and, for an undo or
//! redo, the repository is still as the change (or the last undo) left it: HEAD, the operation
//! in progress and the commits it names, each file's index entries and bytes, and every blob
//! the put-back needs.

use super::aborted::GITLINK;
use super::marked::{IndexAt, PathState, WorkAt, index_at, mixed, too_large, work_at};
use super::marked_put::link_stage;
use super::planning::busy_name;
use super::record::head;
use super::{Action, Plan, Refusal};
use crate::error::{Error, Result};
use git2::Repository;
use std::collections::HashSet;
use std::path::Path;

/// The files in the git folder that name the commits an operation in progress works with.
const OP_HEADS: [&str; 6] = ["MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD", "REBASE_HEAD", "rebase-merge/onto", "rebase-apply/onto"];

/// Each of `OP_HEADS` that is there, with what it holds.
pub(super) fn op_heads(repo: &Repository) -> Vec<(String, String)> {
    let read = |f: &str| std::fs::read_to_string(repo.path().join(f)).ok().map(|t| (f.to_owned(), t.trim().to_owned()));
    OP_HEADS.iter().filter_map(|f| read(f)).collect()
}

/// Refuses `paths` when two of them are one file where case doesn't count (`core.ignoreCase`),
/// or when a folder on the way to one is a symbolic link, which a write would go through.
pub(super) fn check_paths(repo: &Repository, paths: &[String]) -> Result<()> {
    let ignore_case = repo.config().and_then(|c| c.get_bool("core.ignoreCase")).unwrap_or(false);
    if ignore_case {
        let mut seen = HashSet::new();
        for p in paths {
            if !seen.insert(p.to_lowercase()) {
                return Err(Error::Unsupported(format!("{p} differs from another conflicted path only in case; resolve them with git")));
            }
        }
    }
    let workdir = repo.workdir().ok_or_else(|| Error::Other("The repository has no working tree".into()))?;
    for p in paths {
        if let Some(dir) = linked_folder(workdir, p) {
            return Err(Error::Unsupported(format!("{dir} is a symbolic link, so Glowfly won't write {p} through it")));
        }
    }
    Ok(())
}

/// The first folder on the way to `path` that is a symbolic link.
fn linked_folder(workdir: &Path, path: &str) -> Option<String> {
    let mut at = String::new();
    for part in path.split('/').take(path.split('/').count().saturating_sub(1)) {
        at = if at.is_empty() { part.to_owned() } else { format!("{at}/{part}") };
        if workdir.join(&at).symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
            return Some(at);
        }
    }
    None
}

/// Refuses a change that would write over `path` (in `before`) when it couldn't be put back:
/// a file whose bytes weren't kept (over 64 MB) or a link no index entry holds.
pub(super) fn restorable(repo: &Repository, path: &str, before: &PathState) -> Result<()> {
    match before.work {
        WorkAt::File(s) if repo.find_blob(s.blob).is_err() => Err(too_large(path)),
        WorkAt::Link(t) if link_stage(&before.index, t).is_none() => Err(Error::Unsupported(format!(
            "No index entry holds the link at {path}, so Glowfly couldn't put it back after writing over it"
        ))),
        _ => Ok(()),
    }
}

fn expired(m: impl std::fmt::Display) -> Plan {
    Plan::Refuse(Refusal::Stale(format!("{m}; removed from the undo history")))
}

/// The blobs putting `want` back needs, missing from the object database (a submodule's
/// commit is never looked up).
fn missing_blob(repo: &Repository, want: &PathState) -> bool {
    let entries: Vec<_> = match &want.index {
        IndexAt::Conflict(stages) => stages.iter().flatten().collect(),
        IndexAt::Merged(s, _) => s.iter().collect(),
    };
    let lost = |o| repo.find_blob(o).is_err();
    entries.iter().any(|s| s.mode != GITLINK && lost(s.blob))
}

/// Plans undoing (or redoing) a `Conflicts` action: refused as stale, with nothing written,
/// when HEAD moved, the operation in progress ended (or another began, or works with other
/// commits), any of its files isn't as the change (or the undo) left it, in the index or on
/// disk, or something the put-back needs is gone. Every refusal drops the entry alone.
pub(super) fn plan(workdir: &Path, action: &Action, redo: bool) -> Result<Plan> {
    let Action::Conflicts { head: at, state, heads, files, .. } = action else { unreachable!("a conflict change") };
    let what = action.describe();
    let repo = Repository::open(workdir)?;
    if head(&repo).1 != *at {
        return Ok(expired(format!("HEAD has moved since the {what}")));
    }
    let now = repo.state();
    if now != *state || op_heads(&repo) != *heads {
        return Ok(expired(match (busy_name(*state), busy_name(now)) {
            (Some(op), _) => format!("The {op} that was in progress during the {what} is over"),
            (None, Some(op)) => format!("A {op} has started since the {what}"),
            (None, None) => format!("The repository has changed since the {what}"),
        }));
    }
    let paths: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
    if let Err(e) = check_paths(&repo, &paths) {
        return Ok(expired(e));
    }
    let index = repo.index()?;
    for f in files {
        let (expect, target) = if redo { (&f.before, &f.after) } else { (&f.after, &f.before) };
        let p = &f.path;
        if mixed(&index, p) || index_at(&index, p) != expect.index {
            return Ok(expired(format!("{p} has changed in the index since")));
        }
        let work = work_at(workdir, p, false)?;
        if work != expect.work {
            return Ok(expired(format!("{p} has changed since")));
        }
        let lost = |what: &str| Ok(expired(format!("Glowfly no longer has {what} of {p}, so it can't be put back")));
        if missing_blob(&repo, target) {
            return lost("the index entries");
        }
        if work == target.work {
            continue;
        }
        let missing = match target.work {
            WorkAt::File(s) => repo.find_blob(s.blob).is_err(),
            WorkAt::Link(t) => link_stage(&target.index, t).is_none(),
            WorkAt::Absent => false,
            WorkAt::Folder => true,
        };
        if missing || work == WorkAt::Folder {
            return lost("a copy of what it held");
        }
    }
    Ok(Plan::PutConflicts)
}

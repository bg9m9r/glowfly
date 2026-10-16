//! Recording an operation: the repository state before it runs, and the `Action` it turned out
//! to be once it has.

use super::{Action, HeadPos, ResetMode, UndoKind, reset};
use crate::error::Result;
use git2::{BranchType, Oid, Repository};
use std::path::Path;

/// Repository state captured before an operation runs (opaque to callers).
#[derive(Debug)]
pub struct Before {
    head: Option<HeadPos>,
    head_oid: Option<Oid>,
    branch: Option<(Oid, Option<String>)>,
    stash_top: Option<Oid>,
}

pub(super) fn head(repo: &Repository) -> (Option<HeadPos>, Option<Oid>) {
    match repo.head() {
        Ok(h) => {
            let oid = h.target();
            let pos = if h.is_branch() { h.shorthand().map(|b| HeadPos::Branch(b.to_owned())) } else { oid.map(HeadPos::Detached) };
            (pos, oid)
        }
        Err(_) => (None, None), // unborn
    }
}

pub(super) fn stash_top(repo: &Repository) -> Option<Oid> {
    repo.find_reference("refs/stash").ok().and_then(|r| r.target())
}

pub(super) fn ref_exists(repo: &Repository, name: &str) -> bool {
    repo.find_reference(name).is_ok()
}

pub(super) fn branch_name(pos: &Option<HeadPos>) -> Option<String> {
    match pos {
        Some(HeadPos::Branch(b)) => Some(b.clone()),
        _ => None,
    }
}

/// Records what an operation of `kind` may change, before it runs.
pub fn before(workdir: &Path, kind: &UndoKind) -> Result<Before> {
    let repo = Repository::open(workdir)?;
    let (head, head_oid) = head(&repo);
    let branch = match kind {
        UndoKind::DeleteBranch(name) => repo.find_branch(name, BranchType::Local).ok().and_then(|b| {
            let oid = b.get().target()?;
            let upstream = b.upstream().ok().and_then(|u| u.name().ok().flatten().map(str::to_owned));
            Some((oid, upstream))
        }),
        _ => None,
    };
    let stash_top = match kind {
        UndoKind::DiscardAll | UndoKind::Reset(ResetMode::Hard) => stash_top(&repo),
        _ => None,
    };
    Ok(Before { head, head_oid, branch, stash_top })
}

/// The action an operation of `kind` performed, or `None` if it changed nothing undoable.
pub fn after(workdir: &Path, kind: UndoKind, before: Before) -> Result<Option<Action>> {
    let repo = Repository::open(workdir)?;
    let (pos, oid) = head(&repo);
    Ok(match kind {
        UndoKind::Checkout => match (before.head, pos) {
            (Some(from), Some(to)) if from != to => Some(Action::Checkout { from, to }),
            _ => None,
        },
        UndoKind::Commit { amend } => match (before.head_oid, oid) {
            (Some(from), Some(to)) if from != to => {
                let message = repo.find_commit(to)?.message().unwrap_or_default().trim().to_owned();
                Some(Action::Commit { branch: branch_name(&pos), from, to, message, amend })
            }
            _ => None,
        },
        UndoKind::Reset(mode) => match (before.head_oid, oid) {
            // A hard reset to HEAD only discards changes, which its stash makes undoable.
            (Some(from), Some(to)) => {
                let stash =
                    if mode == ResetMode::Hard { reset::new_stash(&repo, from) } else { None }.filter(|s| Some(*s) != before.stash_top);
                (from != to || stash.is_some()).then(|| Action::Reset { mode, branch: branch_name(&pos), from, to, stash })
            }
            _ => None,
        },
        UndoKind::DeleteBranch(name) => match before.branch {
            Some((oid, upstream)) if !ref_exists(&repo, &format!("refs/heads/{name}")) => {
                Some(Action::DeleteBranch { name, oid, upstream })
            }
            _ => None,
        },
        UndoKind::DiscardAll => match stash_top(&repo) {
            Some(stash) if Some(stash) != before.stash_top => {
                let head = repo.find_commit(stash)?.parent_id(0)?;
                Some(Action::DiscardAll { stash, head })
            }
            _ => None,
        },
    })
}

/// `action` as it stands after a successful redo: a re-done discard lives in a new stash, and a
/// re-done hard reset stashed whatever was uncommitted then (maybe nothing).
pub fn redone(workdir: &Path, action: Action) -> Result<Action> {
    Ok(match action {
        Action::DiscardAll { head, .. } => {
            let repo = Repository::open(workdir)?;
            match stash_top(&repo) {
                Some(stash) => Action::DiscardAll { stash, head },
                None => return Err(crate::error::Error::NotFound("the stash for the redone discard".into())),
            }
        }
        Action::Reset { mode: ResetMode::Hard, branch, from, to, .. } => {
            let stash = reset::new_stash(&Repository::open(workdir)?, from);
            Action::Reset { mode: ResetMode::Hard, branch, from, to, stash }
        }
        other => other,
    })
}

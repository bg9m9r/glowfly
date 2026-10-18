//! Recording an operation: the repository state before it runs, and the `Action` it turned out
//! to be once it has.

use super::{Action, Created, HeadPos, ResetMode, UndoKind, is_clean, reset, resumed, stash};
use crate::error::{Error, Result};
use git2::{BranchType, Oid, Repository};
use std::collections::HashSet;
use std::path::Path;

/// Repository state captured before an operation runs (opaque to callers).
#[derive(Debug)]
pub struct Before {
    head: Option<HeadPos>,
    head_oid: Option<Oid>,
    branch: Option<(Oid, Option<String>)>,
    stash_top: Option<Oid>,
    /// For a checkout: the local branch names, to tell whether it created one.
    locals: Option<HashSet<String>>,
    /// For a branch or tag creation: whether the ref already existed.
    existed: bool,
    /// For a pop or apply: the stash commit it applies, its list message, and whether the
    /// working tree was clean (`None`: unknown).
    apply: Option<Apply>,
    /// For a commit or a Continue/Skip: the merge, cherry-pick or revert it may finish.
    paused: Option<resumed::Paused>,
    /// For a stash drop: the stash commit it drops and its list message.
    dropped: Option<(Oid, Option<String>)>,
}

#[derive(Debug)]
struct Apply {
    target: Option<Oid>,
    message: Option<String>,
    clean: Option<bool>,
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

/// Local branch `name`'s tip and upstream (a remote-tracking branch such as "origin/x").
fn local_branch(repo: &Repository, name: &str) -> Option<(Oid, Option<String>)> {
    let b = repo.find_branch(name, BranchType::Local).ok()?;
    let oid = b.get().target()?;
    let upstream = b.upstream().ok().and_then(|u| u.name().ok().flatten().map(str::to_owned));
    Some((oid, upstream))
}

fn local_names(repo: &Repository) -> Result<HashSet<String>> {
    let mut names = HashSet::new();
    for b in repo.branches(Some(BranchType::Local))? {
        if let Some(n) = b?.0.name()? {
            names.insert(n.to_owned());
        }
    }
    Ok(names)
}

/// The stash commit `rev` names, and its message in the stash list.
pub(super) fn apply_target(workdir: &Path, rev: &str) -> Result<(Oid, Option<String>)> {
    let mut repo = Repository::open(workdir)?;
    let oid = repo.revparse_single(rev)?.peel_to_commit()?.id();
    let mut message = None;
    repo.stash_foreach(|_, m, id| {
        if *id == oid {
            message = Some(m.to_owned());
        }
        message.is_none()
    })?;
    Ok((oid, message))
}

/// Records what an operation of `kind` may change, before it runs.
pub fn before(workdir: &Path, kind: &UndoKind) -> Result<Before> {
    let repo = Repository::open(workdir)?;
    let (head, head_oid) = head(&repo);
    let mut b =
        Before { head, head_oid, branch: None, stash_top: None, locals: None, existed: false, apply: None, paused: None, dropped: None };
    match kind {
        UndoKind::DeleteBranch(name) => b.branch = local_branch(&repo, name),
        UndoKind::DiscardAll | UndoKind::Reset(ResetMode::Hard) | UndoKind::StashPush { .. } => b.stash_top = stash_top(&repo),
        UndoKind::Checkout => b.locals = Some(local_names(&repo)?),
        UndoKind::CreateBranch(name) => b.existed = ref_exists(&repo, &format!("refs/heads/{name}")),
        UndoKind::CreateTag(name) => b.existed = ref_exists(&repo, &format!("refs/tags/{name}")),
        UndoKind::StashApply { rev, .. } => {
            let found = apply_target(workdir, rev.as_deref().unwrap_or("stash@{0}")).ok();
            let (target, message) = found.map_or((None, None), |(o, m)| (Some(o), m));
            b.apply = Some(Apply { target, message, clean: is_clean(workdir).ok() });
        }
        UndoKind::Commit { amend: false } | UndoKind::Resume(_) => b.paused = resumed::paused(workdir)?,
        UndoKind::StashDrop { rev } => b.dropped = apply_target(workdir, rev.as_deref().unwrap_or("stash@{0}")).ok(),
        UndoKind::Commit { amend: true } | UndoKind::Reset(_) | UndoKind::Move { .. } => {}
    }
    Ok(b)
}

/// Whether the operation `b` was taken for can't be undone however it ends: a pop or apply onto
/// a working tree that already had changes mixes them with the stash's. Call it before `after`.
pub fn irreversible(b: &Before) -> bool {
    b.apply.as_ref().is_some_and(|a| a.clean != Some(true))
}

/// The action an operation of `kind` performed, or `None` if it changed nothing undoable.
pub fn after(workdir: &Path, kind: UndoKind, mut before: Before) -> Result<Option<Action>> {
    let repo = Repository::open(workdir)?;
    let (pos, oid) = head(&repo);
    let moved = match (before.head_oid, oid) {
        (Some(from), Some(to)) if from != to => Some((from, to)),
        _ => None,
    };
    let paused = before.paused.take();
    Ok(match kind {
        UndoKind::Checkout => match (before.head, pos) {
            (Some(from), Some(to)) if from != to => {
                let created = match (&to, &before.locals, oid) {
                    (HeadPos::Branch(b), Some(locals), Some(oid)) if !locals.contains(b) => {
                        let upstream = local_branch(&repo, b).and_then(|(_, u)| u);
                        Some(Created { name: b.clone(), oid, upstream })
                    }
                    _ => None,
                };
                Some(Action::Checkout { from, to, created })
            }
            _ => None,
        },
        // A merge, cherry-pick or revert finished by this commit, or by a Continue or Skip.
        UndoKind::Commit { .. } | UndoKind::Resume(_) if paused.is_some() => match paused {
            Some(p) => resumed::finished(&repo, p, &pos, oid)?,
            None => None,
        },
        UndoKind::Resume(_) => None,
        UndoKind::StashDrop { .. } => match before.dropped {
            Some((stash, message)) if reset::stash_index(workdir, stash)?.is_none() => {
                let message = match message {
                    Some(m) => m,
                    None => repo.find_commit(stash)?.summary().unwrap_or_default().to_owned(),
                };
                Some(Action::StashDrop { stash, message })
            }
            _ => None,
        },
        UndoKind::Commit { amend } => match moved {
            Some((from, to)) => {
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
        UndoKind::CreateBranch(_) | UndoKind::CreateTag(_) if before.existed => None,
        UndoKind::CreateBranch(name) => local_branch(&repo, &name).map(|(oid, upstream)| Action::CreateBranch { name, oid, upstream }),
        UndoKind::CreateTag(name) => {
            let target = repo.find_reference(&format!("refs/tags/{name}")).ok().and_then(|r| r.target());
            target.map(|oid| Action::CreateTag { name, oid })
        }
        UndoKind::Move { op, detail } => moved.map(|(from, to)| Action::Moved { op, detail, branch: branch_name(&pos), from, to }),
        UndoKind::StashPush { replay } => match stash_top(&repo) {
            Some(stash) if Some(stash) != before.stash_top => {
                let c = repo.find_commit(stash)?;
                let message = c.summary().unwrap_or_default().to_owned();
                Some(Action::StashPush { stash, head: c.parent_id(0)?, message, replay })
            }
            _ => None,
        },
        UndoKind::StashApply { pop, .. } => match (before.apply, oid) {
            (Some(Apply { target: Some(stash), message, clean: Some(true) }), Some(head)) => {
                let left = stash::changes(workdir)?;
                let message = match message {
                    Some(m) => m,
                    None => repo.find_commit(stash)?.summary().unwrap_or_default().to_owned(),
                };
                (!left.0.is_empty()).then_some(Action::StashApply { pop, stash, head, message, left })
            }
            _ => None,
        },
    })
}

/// `action` as it stands after a successful redo: a re-done discard lives in a new stash, a
/// re-done hard reset stashed whatever was uncommitted then (maybe nothing), a re-done stash is
/// a new stash commit, and a re-done pop or apply left the working tree as it is now.
pub fn redone(workdir: &Path, action: Action) -> Result<Action> {
    Ok(match action {
        Action::DiscardAll { head, .. } => {
            let repo = Repository::open(workdir)?;
            match stash_top(&repo) {
                Some(stash) => Action::DiscardAll { stash, head },
                None => return Err(Error::NotFound("the stash for the redone discard".into())),
            }
        }
        Action::Reset { mode: ResetMode::Hard, branch, from, to, .. } => {
            let stash = reset::new_stash(&Repository::open(workdir)?, from);
            Action::Reset { mode: ResetMode::Hard, branch, from, to, stash }
        }
        Action::StashPush { head, message, replay, .. } => {
            let repo = Repository::open(workdir)?;
            let top = stash_top(&repo).filter(|s| repo.find_commit(*s).and_then(|c| c.parent_id(0)).ok() == Some(head));
            match top {
                Some(stash) => Action::StashPush { stash, head, message, replay },
                None => return Err(Error::NotFound("the stash for the redone stash".into())),
            }
        }
        Action::StashApply { pop, stash, head, message, .. } => {
            Action::StashApply { pop, stash, head, message, left: stash::changes(workdir)? }
        }
        other => other,
    })
}

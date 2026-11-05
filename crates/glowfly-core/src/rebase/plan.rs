//! The commits an interactive rebase would replay, read with libgit2.

use super::{Busy, RebaseAction, RebaseBase, RebaseError, RebasePlan, TodoItem, TodoStep, Upstream, merges, short, update_refs};
use crate::commit_text::{self, Decoder};
use crate::error::Result;
use git2::{Oid, Repository, Sort};
use std::collections::HashSet;
use std::path::Path;

/// The list for rebasing the current branch from `base`, oldest first, all picks; a range that
/// holds merges gets the label, reset and merge steps that recreate them, and with
/// `rebase.updateRefs` the update-ref steps git adds for the branches inside it. Refused while
/// an operation is in progress, on a detached HEAD, for a commit that isn't on the current
/// branch, and when there is nothing to rebase; a missing branch or commit is
/// `BranchNotFound` or `CommitNotFound`.
pub fn plan(workdir: &Path, base: &RebaseBase) -> Result<RebasePlan, RebaseError> {
    let repo = Repository::open(workdir)?;
    let (head, branch) = checked_head(&repo)?;
    let (upstream, onto_label) = match base {
        RebaseBase::FromCommit(c) => {
            if *c != head && !repo.graph_descendant_of(head, *c)? {
                return Err(RebaseError::CommitNotOnBranch { commit: short(*c) });
            }
            let commit = repo.find_commit(*c)?;
            match commit.parent_ids().next() {
                Some(p) => {
                    let summary = commit_text::summary(&repo.find_commit(p)?).into_owned();
                    (Upstream::Commit(p), format!("{} {summary}", short(p)).trim_end().to_owned())
                }
                None => (Upstream::Root, String::new()),
            }
        }
        RebaseBase::OntoBranch(name) => {
            let tip = repo
                .find_reference(&format!("refs/heads/{name}"))
                .and_then(|r| r.peel_to_commit())
                .map_err(|_| RebaseError::BranchNotFound { name: name.clone() })?
                .id();
            (Upstream::Commit(tip), name.clone())
        }
        RebaseBase::OntoCommit(o) => {
            let commit = repo.find_commit(*o).map_err(|_| RebaseError::CommitNotFound { commit: short(*o) })?;
            let label = format!("{} {}", short(*o), commit_text::summary(&commit)).trim_end().to_owned();
            (Upstream::Commit(*o), label)
        }
    };
    let oids = range(&repo, head, upstream)?;
    if oids.is_empty() {
        return Err(RebaseError::NothingToRebase { branch, onto: onto_label });
    }
    let mut has_merge = false;
    for o in &oids {
        has_merge |= repo.find_commit(*o)?.parent_count() > 1;
    }
    let steps = if has_merge {
        merges::script(&repo, head, upstream, &oids)?
    } else {
        oids.into_iter().map(|o| item(&repo, o).map(TodoStep::Commit)).collect::<Result<Vec<_>>>()?
    };
    let in_range: HashSet<Oid> = steps.iter().filter_map(TodoStep::oid).collect();
    let tips = update_refs::predict(&repo, workdir, &format!("refs/heads/{branch}"), &in_range)?;
    let steps = update_refs::add(steps, &tips);
    Ok(RebasePlan { head, branch, upstream, onto_label, steps, held: tips.held })
}

/// HEAD's commit and branch, refusing while an operation is in progress or on a detached or
/// unborn HEAD.
pub(super) fn checked_head(repo: &Repository) -> Result<(Oid, String), RebaseError> {
    if let Some(op) = Busy::of(repo.state()) {
        return Err(RebaseError::InProgress { op });
    }
    if repo.head_detached()? {
        return Err(RebaseError::Detached);
    }
    let head = repo.head().map_err(|_| RebaseError::NoCommitsYet)?;
    let branch = head.shorthand().unwrap_or_default().to_owned();
    let oid = head.target().ok_or(RebaseError::NoCommitsYet)?;
    Ok((oid, branch))
}

/// The commits from `upstream` (exclusive) to `head`, merges included, oldest first: the set
/// git would rebase.
pub(super) fn range(repo: &Repository, head: Oid, upstream: Upstream) -> Result<Vec<Oid>> {
    let mut walk = repo.revwalk()?;
    walk.push(head)?;
    if let Upstream::Commit(u) = upstream {
        walk.hide(u)?;
    }
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;
    let mut out = Vec::new();
    for oid in walk {
        out.push(oid?);
    }
    Ok(out)
}

/// A pick of the commit `oid`.
pub(super) fn item(repo: &Repository, oid: Oid) -> Result<TodoItem> {
    let c = repo.find_commit(oid)?;
    let (d, sig) = (Decoder::of(&c), c.author());
    Ok(TodoItem {
        oid,
        action: RebaseAction::Pick,
        summary: d.summary(&c).into_owned(),
        message: d.message(&c).into_owned(),
        new_message: None,
        combined_message: None,
        author: d.name(&sig).into_owned(),
        time: c.time().seconds(),
    })
}

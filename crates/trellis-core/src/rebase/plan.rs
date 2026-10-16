//! The commits an interactive rebase would replay, read with libgit2.

use super::{DETACHED, RebaseAction, RebaseBase, RebasePlan, TodoItem, Upstream, in_progress_text, short};
use crate::error::{Error, Result};
use git2::{Oid, Repository, Sort};
use std::path::Path;

/// The list for rebasing the current branch from `base`, oldest first, all picks. Refused
/// (`Error::Unsupported`, with a message for the user) while an operation is in progress, on a
/// detached HEAD, for a commit that isn't on the current branch, when the range holds a merge,
/// and when there is nothing to rebase. A missing branch is `Error::NotFound`.
pub fn plan(workdir: &Path, base: &RebaseBase) -> Result<RebasePlan> {
    let repo = Repository::open(workdir)?;
    let (head, branch) = checked_head(&repo)?;
    let (upstream, onto_label) = match base {
        RebaseBase::FromCommit(c) => {
            if *c != head && !repo.graph_descendant_of(head, *c)? {
                return Err(Error::Unsupported(format!("{} is not on the current branch", short(*c))));
            }
            let commit = repo.find_commit(*c)?;
            if commit.parent_count() > 1 {
                return Err(merge_refused(*c));
            }
            match commit.parent_ids().next() {
                Some(p) => {
                    let summary = repo.find_commit(p)?.summary().unwrap_or_default().to_owned();
                    (Upstream::Commit(p), format!("{} {summary}", short(p)).trim_end().to_owned())
                }
                None => (Upstream::Root, String::new()),
            }
        }
        RebaseBase::OntoBranch(name) => {
            let tip = repo
                .find_reference(&format!("refs/heads/{name}"))
                .and_then(|r| r.peel_to_commit())
                .map_err(|_| Error::NotFound(format!("branch {name}")))?
                .id();
            (Upstream::Commit(tip), name.clone())
        }
    };
    let oids = range(&repo, head, upstream)?;
    if oids.is_empty() {
        return Err(Error::Unsupported(format!("{branch} has no commits that aren't already on {onto_label}")));
    }
    let items = oids.into_iter().map(|o| item(&repo, o)).collect::<Result<Vec<_>>>()?;
    Ok(RebasePlan { head, branch, upstream, onto_label, items })
}

/// HEAD's commit and branch, refusing while an operation is in progress or on a detached or
/// unborn HEAD.
pub(super) fn checked_head(repo: &Repository) -> Result<(Oid, String)> {
    if let Some(text) = in_progress_text(repo.state()) {
        return Err(Error::Unsupported(text));
    }
    if repo.head_detached()? {
        return Err(Error::Unsupported(DETACHED.into()));
    }
    let head = repo.head().map_err(|_| Error::Unsupported("The branch has no commits yet.".into()))?;
    let branch = head.shorthand().unwrap_or_default().to_owned();
    let oid = head.target().ok_or_else(|| Error::Unsupported("The branch has no commits yet.".into()))?;
    Ok((oid, branch))
}

/// The commits from `upstream` (exclusive) to `head`, oldest first: the set git would list.
/// Refused when one of them is a merge.
pub(super) fn range(repo: &Repository, head: Oid, upstream: Upstream) -> Result<Vec<Oid>> {
    let mut walk = repo.revwalk()?;
    walk.push(head)?;
    if let Upstream::Commit(u) = upstream {
        walk.hide(u)?;
    }
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;
    let mut out = Vec::new();
    for oid in walk {
        let oid = oid?;
        if repo.find_commit(oid)?.parent_count() > 1 {
            return Err(merge_refused(oid));
        }
        out.push(oid);
    }
    Ok(out)
}

pub(super) fn merge_refused(oid: Oid) -> Error {
    Error::Unsupported(format!("The range includes the merge commit {}; rebasing merges isn't supported yet", short(oid)))
}

fn item(repo: &Repository, oid: Oid) -> Result<TodoItem> {
    let c = repo.find_commit(oid)?;
    Ok(TodoItem {
        oid,
        action: RebaseAction::Pick,
        summary: c.summary().unwrap_or_default().to_owned(),
        message: c.message().unwrap_or_default().to_owned(),
        new_message: None,
        author: c.author().name().unwrap_or_default().to_owned(),
        time: c.time().seconds(),
    })
}

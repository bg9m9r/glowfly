//! How many commits deleting a local branch would leave on no branch: those its tip reaches
//! that no other local branch and no remote-tracking branch (its upstream included) reaches.
//! The branch Delete dialog asks for this before it offers Delete or Delete anyway.

use crate::error::Result;
use git2::{Oid, Repository};
use std::path::Path;

/// The commits reachable from `tip` (local branch `name`'s tip) but from no other branch under
/// `refs/heads/` or `refs/remotes/`. Tags, stashes and a detached HEAD don't count: a commit only
/// they reach is still off every branch.
pub fn unmerged_count(workdir: &Path, name: &str, tip: Oid) -> Result<usize> {
    let repo = Repository::open(workdir)?;
    let own = format!("refs/heads/{name}");
    let mut walk = repo.revwalk()?;
    walk.push(tip)?;
    for r in repo.references()? {
        let r = r?;
        let Some(full) = r.name() else { continue };
        if full == own || !(full.starts_with("refs/heads/") || full.starts_with("refs/remotes/")) {
            continue;
        }
        // A remote's symbolic HEAD resolves to one of its branches; a ref that isn't a commit
        // can't hold the branch's commits.
        if let Ok(c) = r.peel_to_commit() {
            walk.hide(c.id())?;
        }
    }
    let mut n = 0;
    for oid in walk {
        oid?;
        n += 1;
    }
    Ok(n)
}

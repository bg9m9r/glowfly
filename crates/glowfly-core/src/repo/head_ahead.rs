//! The checked-out branch's commits strictly ahead of its upstream along first parents
//! (`Snapshot::head_ahead`): what the Tidy tip counts and where its rebase starts. `ahead`
//! counts every commit the upstream lacks, a locally merged side branch's too, so walking that
//! many first parents can run into pushed commits; this walk stops at the first commit the
//! upstream has.

use crate::repo::BranchInfo;
use git2::{Oid, Repository};
use std::collections::HashSet;

/// Most commits ahead worked out; a branch further ahead gets None.
const MAX_AHEAD: usize = 1000;

/// HEAD's first-parent commits that its branch's upstream doesn't have, newest first; None when
/// HEAD isn't on a branch with an upstream, or the answer is unknown or too long.
pub(super) fn first_parent(repo: &Repository, head: Option<Oid>, locals: &[BranchInfo]) -> Option<Vec<Oid>> {
    let head = head?;
    let b = locals.iter().find(|b| b.is_head && b.oid == Some(head))?;
    let upstream = repo.refname_to_id(b.upstream_ref.as_deref()?).ok()?;
    if b.ahead == 0 {
        return Some(Vec::new());
    }
    if b.ahead > MAX_AHEAD {
        return None;
    }
    let mut walk = repo.revwalk().ok()?;
    walk.push(head).ok()?;
    walk.hide(upstream).ok()?;
    let not_pushed: HashSet<Oid> = walk.take(MAX_AHEAD + 1).collect::<Result<_, _>>().ok()?;
    let mut out = Vec::new();
    let mut at = Some(head);
    while let Some(oid) = at.filter(|o| not_pushed.contains(o)) {
        out.push(oid);
        at = repo.find_commit(oid).ok()?.parent_id(0).ok();
    }
    Some(out)
}

#[cfg(test)]
#[path = "head_ahead_tests.rs"]
mod tests;

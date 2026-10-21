//! Which of the journal's commits nothing keeps any more: a revwalk that starts at the rows'
//! commits and hides every ref under `refs/`, HEAD of this and every other worktree, and every
//! entry of the stash list. What it yields is reachable from a row and from none of those.
//! Problems (a ref that won't peel, a worktree that won't open, a missing parent in a shallow
//! clone) never fail the journal's load; they only leave rows unmarked.

use git2::{Oid, Repository, Revwalk};
use std::collections::HashSet;

/// The most commits the walk visits before it stops (the rows it didn't reach stay unmarked).
pub const MAX_WALK: usize = 100_000;

/// The commits among `oids` that no ref, worktree HEAD or stash entry reaches. Commits that are
/// gone are never in it.
pub(super) fn unreachable(repo: &Repository, oids: impl Iterator<Item = Oid>) -> HashSet<Oid> {
    let mut out = HashSet::new();
    let Ok(mut walk) = repo.revwalk() else { return out };
    let mut wanted = HashSet::new();
    for oid in oids {
        if !wanted.contains(&oid) && repo.find_commit(oid).is_ok() && walk.push(oid).is_ok() {
            wanted.insert(oid);
        }
    }
    if wanted.is_empty() {
        return out;
    }
    hide_tips(repo, &mut walk);
    for id in walk.take(MAX_WALK) {
        let Ok(id) = id else { break };
        if wanted.contains(&id) {
            out.insert(id);
        }
    }
    out
}

/// Hides what keeps commits: every ref, each worktree's HEAD and the stash list's entries.
fn hide_tips(repo: &Repository, walk: &mut Revwalk<'_>) {
    let mut hide = |oid: Oid| {
        let _ = walk.hide(oid);
    };
    if let Ok(refs) = repo.references() {
        for r in refs.flatten() {
            if let Ok(c) = r.peel_to_commit() {
                hide(c.id());
            }
        }
    }
    let head_of = |r: &Repository| r.head().ok().and_then(|h| h.peel_to_commit().ok()).map(|c| c.id());
    if let Some(h) = head_of(repo) {
        hide(h);
    }
    // From a linked worktree, `worktrees()` lists only the linked ones: open the main one too.
    if let Ok(main) = Repository::open(repo.commondir())
        && !main.is_bare()
        && let Some(h) = head_of(&main)
    {
        hide(h);
    }
    if let Ok(names) = repo.worktrees() {
        for name in names.iter().flatten() {
            let Ok(wt) = repo.find_worktree(name) else { continue };
            let Ok(r) = Repository::open_from_worktree(&wt) else { continue };
            if let Some(h) = head_of(&r) {
                hide(h);
            }
        }
    }
    if let Ok(log) = repo.reflog("refs/stash") {
        for e in log.iter() {
            if repo.find_commit(e.id_new()).is_ok() {
                hide(e.id_new());
            }
        }
    }
}

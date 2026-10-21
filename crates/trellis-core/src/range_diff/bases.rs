//! Where each side of a comparison starts and ends: the tip a ref had before its last rewrite
//! (read from its reflog with libgit2), the base of each range (cut at a mainline when one is
//! known, else at the two versions' merge base), and choosing that mainline from a loaded
//! snapshot.

use super::{Range, Rewrite};
use crate::error::Result;
use crate::journal::MAX_PER_REF;
use crate::repo::Snapshot;
use git2::{ErrorCode, Oid, Repository};

/// The newest entry of `refname`'s reflog that moved it somewhere its old tip doesn't lead to
/// (a rebase, amend, reset, forced fetch or force push). Fast-forwards, creations, entries whose
/// old tip is the tip now and entries whose commits are gone are skipped. None when there is
/// none in the newest `MAX_PER_REF` entries (or no reflog).
pub fn last_rewrite(repo: &Repository, refname: &str) -> Result<Option<Rewrite>> {
    let tip = repo.refname_to_id(refname)?;
    let Ok(log) = repo.reflog(refname) else { return Ok(None) };
    for e in log.iter().take(MAX_PER_REF) {
        let (old, new) = (e.id_old(), e.id_new());
        if old.is_zero() || new.is_zero() || old == tip {
            continue;
        }
        if repo.find_commit(old).is_err() || repo.find_commit(new).is_err() {
            continue;
        }
        if new == old || repo.graph_descendant_of(new, old)? {
            continue;
        }
        let message = String::from_utf8_lossy(e.message_bytes().unwrap_or_default()).into_owned();
        return Ok(Some(Rewrite { before: old, at: e.committer().when().seconds(), message }));
    }
    Ok(None)
}

/// The two ranges to compare, or that the versions share no history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Cut {
    /// Old, new, and whether the mainline cut them (else their merge base did).
    Ranges(Range, Range, bool),
    NoShared,
}

/// The merge base of `a` and `b`; None when they share no history.
fn merge_base(repo: &Repository, a: Oid, b: Oid) -> Result<Option<Oid>> {
    match repo.merge_base(a, b) {
        Ok(o) => Ok(Some(o)),
        Err(e) if e.code() == ErrorCode::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Each side's own commits: since where it left `mainline` when that gives both sides commits
/// of their own, otherwise since the two versions split (the same as `old...new`).
pub(super) fn ranges(repo: &Repository, old: Oid, new: Oid, mainline: Option<Oid>) -> Result<Cut> {
    if let Some(m) = mainline.filter(|m| *m != old && *m != new)
        && let (Some(ob), Some(nb)) = (merge_base(repo, old, m)?, merge_base(repo, new, m)?)
        && ob != old
        && nb != new
    {
        return Ok(Cut::Ranges(Range { base: ob, tip: old }, Range { base: nb, tip: new }, true));
    }
    Ok(match merge_base(repo, old, new)? {
        Some(b) => Cut::Ranges(Range { base: b, tip: old }, Range { base: b, tip: new }, false),
        None => Cut::NoShared,
    })
}

/// The branch the two versions grew from, by name and tip, from the loaded snapshot alone: on
/// `remote` (when it exists, else `origin`, else the first remote) its recorded HEAD branch,
/// then `main`, then `master`; then the local `main` and `master`. The first that exists and
/// isn't named in `exclude` wins.
pub fn mainline(snap: &Snapshot, remote: Option<&str>, exclude: &[&str]) -> Option<(String, Oid)> {
    let names = || snap.remotes.iter().map(|(r, _)| r.as_str());
    let remote = remote.and_then(|w| names().find(|r| *r == w)).or_else(|| names().find(|r| *r == "origin")).or_else(|| names().next());
    let mut candidates: Vec<(String, Option<Oid>)> = Vec::new();
    if let Some(r) = remote {
        let recorded = snap.remote_heads.iter().find(|(name, _)| name == r).map(|(_, b)| b.as_str());
        for b in recorded.into_iter().chain(["main", "master"]) {
            candidates.push((format!("{r}/{b}"), snap.remote_oid(r, b)));
        }
    }
    for b in ["main", "master"] {
        candidates.push((b.to_owned(), snap.local_oid(b)));
    }
    candidates.into_iter().find_map(|(name, oid)| (!exclude.contains(&name.as_str())).then_some((name, oid?)))
}

#[cfg(test)]
#[path = "bases_tests.rs"]
mod tests;

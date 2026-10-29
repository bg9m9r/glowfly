//! What a merge, cherry-pick or revert leaves where hooks follow the working tree. Each commit
//! it takes is compared by itself (compare.rs), but a three-way merge of two trusted versions
//! can make a third that neither is. When the versions it combines differ there, the merges
//! are made in memory (`Repository::merge_trees`, objects only: no checkout, hook, filter or
//! fsmonitor runs) and the version each one leaves is compared too. A conflict in a place
//! hooks run from leaves conflict markers in a hook: it is held as Conflicted.

use super::compare::Refused;
use super::incoming::{Target, Unchecked, allows, listed};
use super::{Follow, TrustScan, follow};
use crate::settings::SavedTrust;
use git2::{Oid, Repository, Tree};

/// One three-way merge git makes: the changes from `base` (None: an empty tree) to `theirs`,
/// applied to the result so far (HEAD's tree at first).
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub base: Option<Oid>,
    pub theirs: Oid,
}

/// The merges `target` makes on HEAD, in order; empty for one that makes none (a checkout,
/// a rebase: compared commit by commit).
pub fn steps(repo: &Repository, target: &Target) -> Result<Vec<Step>, Unchecked> {
    let Ok(head) = repo.head().and_then(|h| h.peel_to_commit()) else { return Ok(Vec::new()) };
    let err = |e: git2::Error| Unchecked::Unreadable(e.message().to_owned());
    match target {
        Target::Merged(_) => {
            let heads = super::incoming::commits(repo, &[], target)?;
            Ok(heads.into_iter().map(|theirs| Step { base: repo.merge_base(head.id(), theirs).ok(), theirs }).collect())
        }
        Target::Picked { revs, revert } => {
            let mut picked = Vec::new();
            for r in revs {
                if r.contains("..") || r.starts_with('^') {
                    let mut w = repo.revwalk().map_err(err)?;
                    w.push_range(r).map_err(|_| Unchecked::Unresolved(r.clone()))?;
                    picked.extend(listed(w)?);
                } else {
                    picked.extend(super::incoming::commits(repo, &[], &Target::Rev(r.clone()))?);
                }
            }
            let mut out = Vec::new();
            for c in picked {
                let parent = repo.find_commit(c).map_err(err)?.parent_id(0).ok();
                out.push(match (revert, parent) {
                    (true, Some(p)) => Step { base: Some(c), theirs: p },
                    (true, None) => return Err(Unchecked::Unresolved(c.to_string())),
                    (false, base) => Step { base, theirs: c },
                });
            }
            Ok(out)
        }
        _ => Ok(Vec::new()),
    }
}

/// Whether `saved` covers what each of `steps` leaves where `follows` points, starting from
/// HEAD. Nothing is merged when every version they combine is HEAD's (the result is HEAD's
/// too). A conflict outside those places stops the comparison where git would stop.
pub fn check(repo: &Repository, follows: &[Follow], saved: Option<&SavedTrust>, steps: &[Step]) -> Result<(), Refused> {
    if follows.is_empty() || steps.is_empty() {
        return Ok(());
    }
    let err = |e: git2::Error| Refused::Unchecked(Unchecked::Unreadable(e.message().to_owned()));
    let Ok(head) = repo.head().and_then(|h| h.peel_to_tree()) else { return Ok(()) };
    let at = |oid: Oid| follow::versions(repo, follows, oid);
    let mine = at(head.id()).map_err(err)?;
    let mut same = true;
    for s in steps {
        let theirs = at(s.theirs).map_err(err)?;
        let base = match s.base {
            Some(b) => at(b).map_err(err)?,
            None => follows.iter().map(|f| Follow { tracked: None, ..f.clone() }).collect(),
        };
        same &= theirs == mine && base == mine;
    }
    if same {
        return Ok(());
    }
    let empty = repo.find_tree(repo.treebuilder(None).and_then(|b| b.write()).map_err(err)?).map_err(err)?;
    let tree = |oid: Oid| -> Result<Tree<'_>, git2::Error> { repo.find_object(oid, None)?.peel_to_tree() };
    let mut ours = head;
    for s in steps {
        let base = match s.base {
            Some(b) => tree(b).map_err(err)?,
            None => empty.clone(),
        };
        let mut merged = repo.merge_trees(&base, &ours, &tree(s.theirs).map_err(err)?, None).map_err(err)?;
        if merged.has_conflicts() {
            let conflicts = merged.conflicts().map_err(err)?;
            for c in conflicts.flatten() {
                let sides = [c.ancestor, c.our, c.their];
                let paths = sides.iter().flatten().map(|e| String::from_utf8_lossy(&e.path).into_owned());
                if let Some(p) = paths.into_iter().find(|p| follow::touches(p, follows)) {
                    return Err(Refused::Unchecked(Unchecked::Conflicted(p)));
                }
            }
            // git stops here; the rest is compared when it goes on (the sequencer's steps).
            return Ok(());
        }
        let id = merged.write_tree_to(repo).map_err(err)?;
        let versions = at(id).map_err(err)?;
        if !allows(saved, &TrustScan { follows: versions, ..TrustScan::default() }) {
            return Err(Refused::Held(Box::new(follow::incoming(repo, follows, id).map_err(err)?)));
        }
        ours = repo.find_tree(id).map_err(err)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "merged_tests.rs"]
mod tests;

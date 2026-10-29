//! What a merge, pick, revert, rebase, stash or the sequencer leaves where hooks follow the
//! working tree. Each commit it takes is compared by itself (compare.rs), but a three-way merge
//! of two trusted versions can make a third that neither is. So each merge git would make is
//! made by git itself, objects only (`git merge-tree --write-tree`, plumbing.rs: ort with the
//! command's `-X` options and git's config, criss-cross bases found as git finds them; no
//! checkout, hook, filter or fsmonitor runs), one after another as the command would make
//! them, and the version each leaves is compared too. A conflict in a place hooks run from
//! leaves conflict markers in a hook: it is held as Conflicted. What can't be made that way
//! (an octopus merge, a todo that merges or resets, an older git) is Unmodeled.

use super::compare::Refused;
use super::incoming::{self, How, Target, Unchecked, allows};
use super::{Follow, TrustScan, follow, plumbing};
use crate::settings::SavedTrust;
use git2::{Oid, Repository};

/// One three-way merge: the changes from `base` (None: the merge bases git finds, for a merge
/// of two commits) to `theirs`, made on the result so far.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub base: Option<Oid>,
    pub theirs: Oid,
}

/// The merges a command makes, in order, from `ours` (a commit, or a tree), with `how`'s
/// `-X` options.
#[derive(Clone, Debug, PartialEq)]
pub struct Chain {
    pub ours: Oid,
    pub steps: Vec<Step>,
    pub how: How,
}

fn unreadable(e: git2::Error) -> Unchecked {
    Unchecked::Unreadable(e.message().to_owned())
}

/// The tree of `oid` (a commit or a tree).
fn tree_of(repo: &Repository, oid: Oid) -> Result<Oid, Unchecked> {
    repo.find_object(oid, None).and_then(|o| o.peel_to_tree()).map(|t| t.id()).map_err(unreadable)
}

fn empty_tree(repo: &Repository) -> Result<Oid, Unchecked> {
    repo.treebuilder(None).and_then(|b| b.write()).map_err(unreadable)
}

/// Each pick of `commits` (a revert: back to its parent) as a step: the changes from its
/// parent (`-m`'s, for a merge commit; an empty tree for a root) to it.
pub fn pick_steps(repo: &Repository, commits: &[Oid], revert: bool, mainline: Option<u32>) -> Result<Vec<Step>, Unchecked> {
    let mut out = Vec::new();
    for c in commits {
        let commit = repo.find_commit(*c).map_err(unreadable)?;
        let parent = match (commit.parent_count(), mainline) {
            (0, _) => empty_tree(repo)?,
            (1, _) => tree_of(repo, commit.parent_id(0).map_err(unreadable)?)?,
            (_, Some(m)) if m >= 1 => {
                let p = commit.parent_id(m as usize - 1).map_err(|_| Unchecked::Unresolved(format!("-m {m}")))?;
                tree_of(repo, p)?
            }
            // git refuses a merge commit without -m.
            _ => return Err(Unchecked::Unresolved(format!("{c} (a merge, without -m)"))),
        };
        let own = tree_of(repo, *c)?;
        out.push(if revert { Step { base: Some(own), theirs: parent } } else { Step { base: Some(parent), theirs: own } });
    }
    Ok(out)
}

/// The tree HEAD has now.
fn head_tree(repo: &Repository) -> Result<Oid, Unchecked> {
    repo.head().and_then(|h| h.peel_to_tree()).map(|t| t.id()).map_err(unreadable)
}

fn checked_strategy(how: &How) -> Result<(), Unchecked> {
    if how.strategy.as_deref() == Some("recursive") {
        // `recursive` is made by ort from git 2.50 on; before it, by the old merge.
        plumbing::needs(2, 50, "comparing -s recursive")?;
    }
    Ok(())
}

/// The merges `target` makes, in order; None for one that makes none (a checkout: compared
/// as its commits are), or makes HEAD's tree (`merge -s ours`).
pub fn steps(repo: &Repository, target: &Target) -> Result<Option<Chain>, Unchecked> {
    match target {
        Target::Merged { how, .. } if how.strategy.as_deref() == Some("ours") => Ok(None),
        Target::Merged { revs, how } => {
            checked_strategy(how)?;
            // Into an unborn branch a merge checks the commit out: its commits are compared.
            let head = match repo.head() {
                Err(e) if e.code() == git2::ErrorCode::UnbornBranch => return Ok(None),
                head => head.and_then(|h| h.peel_to_commit()).map_err(unreadable)?,
            };
            match incoming::merge_heads(repo, revs)?.as_slice() {
                [] => Ok(None),
                [theirs] => Ok(Some(Chain { ours: head.id(), steps: vec![Step { base: None, theirs: *theirs }], how: how.clone() })),
                _ => Err(Unchecked::Unmodeled("an octopus merge".to_owned())),
            }
        }
        Target::Picked { revs, revert, how } => {
            checked_strategy(how)?;
            let picked = incoming::picks(repo, revs, *revert, how)?;
            let steps = pick_steps(repo, &picked, *revert, how.mainline)?;
            Ok(Some(Chain { ours: head_tree(repo)?, steps, how: how.clone() }))
        }
        Target::Replayed { how, .. } => {
            checked_strategy(how)?;
            let r = incoming::replay(repo, target)?;
            let ours = match r.onto {
                Some(o) => tree_of(repo, o)?,
                None => empty_tree(repo)?,
            };
            Ok(Some(Chain { ours, steps: pick_steps(repo, &r.picks, false, None)?, how: how.clone() }))
        }
        Target::Steps { onto, picks } => {
            let picks = picks.as_ref().ok_or_else(|| Unchecked::Unmodeled("a rebase that merges or resets".to_owned()))?;
            let ours = match onto {
                Some(o) => tree_of(repo, *o)?,
                None => empty_tree(repo)?,
            };
            Ok(Some(Chain { ours, steps: pick_steps(repo, picks, false, None)?, how: How::default() }))
        }
        Target::Stash { spec, .. } => {
            let stash = repo.find_commit(incoming::commit(repo, spec)?).map_err(unreadable)?;
            let base = tree_of(repo, stash.parent_id(0).map_err(unreadable)?)?;
            let step = Step { base: Some(base), theirs: stash.tree_id() };
            Ok(Some(Chain { ours: head_tree(repo)?, steps: vec![step], how: How::default() }))
        }
        Target::Sequencer { skip } => {
            let todo = incoming::sequencer::read(repo)?;
            let picks = todo.picks.ok_or_else(|| Unchecked::Unmodeled("a todo that merges or resets".to_owned()))?;
            if picks.is_empty() {
                return Ok(None);
            }
            // `--continue` commits the index first; `--skip` goes back to HEAD.
            let ours = if *skip { head_tree(repo)? } else { repo.index().and_then(|mut i| i.write_tree()).map_err(unreadable)? };
            let mut steps = Vec::new();
            for p in picks {
                steps.extend(pick_steps(repo, &[p.commit], p.revert, None)?);
            }
            Ok(Some(Chain { ours, steps, how: How::default() }))
        }
        _ => Ok(None),
    }
}

/// Whether `saved` covers what each of `chain`'s merges leaves where `follows` points. Nothing
/// is merged when every version they combine is the same there (the result is that one too);
/// after a conflict elsewhere git stops, so the steps after it aren't made.
pub fn check(repo: &Repository, follows: &[Follow], saved: Option<&SavedTrust>, chain: &Chain) -> Result<(), Refused> {
    if follows.is_empty() || chain.steps.is_empty() {
        return Ok(());
    }
    let unchecked = |e: git2::Error| Refused::Unchecked(unreadable(e));
    let at = |oid: Oid| follow::versions(repo, follows, oid);
    let mine = at(chain.ours).map_err(unchecked)?;
    let mut same = true;
    for s in &chain.steps {
        same &= at(s.theirs).map_err(unchecked)? == mine;
        let bases: Vec<Oid> = match s.base {
            Some(b) => vec![b],
            // Bases that can't be listed: merged, not assumed.
            None => repo.merge_bases(chain.ours, s.theirs).map(|b| b.iter().copied().collect()).unwrap_or_default(),
        };
        same &= !bases.is_empty();
        for b in bases {
            same &= at(b).map_err(unchecked)? == mine;
        }
    }
    if same {
        return Ok(());
    }
    let mut ours = chain.ours;
    for s in &chain.steps {
        let merged =
            plumbing::merge_tree(repo, ours, s.theirs, s.base, &chain.how.xopts, chain.how.unrelated).map_err(Refused::Unchecked)?;
        if let Some(p) = merged.conflicts.iter().find(|p| follow::touches(p, follows)) {
            return Err(Refused::Unchecked(Unchecked::Conflicted(p.clone())));
        }
        if !allows(saved, &TrustScan { follows: at(merged.tree).map_err(unchecked)?, ..TrustScan::default() }) {
            return Err(Refused::Held(Box::new(follow::incoming(repo, follows, merged.tree).map_err(unchecked)?)));
        }
        if !merged.conflicts.is_empty() {
            // git stops here, with the files as merged so far (compared above).
            return Ok(());
        }
        ours = merged.tree;
    }
    Ok(())
}

#[cfg(test)]
#[path = "merged_tests.rs"]
mod tests;

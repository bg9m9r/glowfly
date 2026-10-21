//! Reading a repository's stacks with libgit2: the base (the main branch, Git Flow's production
//! and development branches, and every remote's branch of those names), the commits of the
//! other local branches outside it, and each branch's former tips from its reflog.

use super::link::{BranchTip, StackInput, link};
use super::{MAX_REFLOG, MAX_WALK, Stacks};
use crate::repo::Snapshot;
use git2::{Oid, Repository};
use std::collections::HashMap;

/// Reflog messages that leave a commit the branch was built on: a commit, a rebase (also a
/// branch `--update-refs` moved), a cherry-pick, a revert or `git am`. "branch:" (created,
/// `branch -f`), "reset:", "merge" and "pull" entries are left out, so a branch created at
/// another branch's tip doesn't look like its lower layer.
const BUILT: [&str; 5] = ["commit", "rebase", "cherry-pick", "revert", "am"];

/// The stacks of the repository `snap` was loaded from. Empty without a main branch, with an
/// unborn HEAD, or on any error.
pub fn read(repo: &Repository, snap: &Snapshot) -> Stacks {
    input(repo, snap).map(|i| link(&i)).unwrap_or_default()
}

fn input(repo: &Repository, snap: &Snapshot) -> Option<StackInput> {
    snap.head_oid?;
    let mut bases = vec![snap.mainline()?];
    if let Some(flow) = snap.gitflow.settings() {
        for b in [flow.production, flow.development] {
            if !b.is_empty() && !bases.contains(&b) {
                bases.push(b);
            }
        }
    }
    let mut base_tips: Vec<Oid> = bases.iter().filter_map(|b| snap.local_oid(b)).collect();
    for (_, list) in &snap.remotes {
        base_tips.extend(list.iter().filter(|(n, _)| bases.contains(n)).map(|(_, o)| *o));
    }
    let candidates: Vec<(&str, Oid)> =
        snap.locals.iter().filter(|b| !bases.contains(&b.name)).filter_map(|b| Some((b.name.as_str(), b.oid?))).collect();
    if candidates.is_empty() {
        return Some(StackInput::default());
    }

    // Hiding marks every parent, second ones included, so a branch merged through a merge's
    // second parent counts as merged; the first parents are followed by hand afterwards.
    let mut walk = repo.revwalk().ok()?;
    for (_, tip) in &candidates {
        let _ = walk.push(*tip);
    }
    for tip in &base_tips {
        let _ = walk.hide(*tip);
    }
    let mut first_parent = HashMap::new();
    for oid in walk.flatten().take(MAX_WALK) {
        let parent = repo.find_commit(oid).ok().and_then(|c| c.parent_id(0).ok());
        first_parent.insert(oid, parent);
    }

    let branches = candidates
        .into_iter()
        .filter(|(_, tip)| first_parent.contains_key(tip))
        .map(|(name, tip)| BranchTip { name: name.to_owned(), tip, former: former_tips(repo, name, tip) })
        .collect();
    Some(StackInput { branches, first_parent })
}

/// The commits branch `name` pointed at before, newest first, from its newest `MAX_REFLOG`
/// entries that built on it (`BUILT`); `tip` and duplicates left out.
fn former_tips(repo: &Repository, name: &str, tip: Oid) -> Vec<Oid> {
    let Ok(log) = repo.reflog(&format!("refs/heads/{name}")) else { return Vec::new() };
    let mut out: Vec<Oid> = Vec::new();
    for entry in log.iter().filter(|e| e.message().is_some_and(|m| BUILT.iter().any(|p| m.starts_with(p)))).take(MAX_REFLOG) {
        let id = entry.id_new();
        if !id.is_zero() && id != tip && !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

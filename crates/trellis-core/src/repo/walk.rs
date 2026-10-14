//! Reading the newest commits for the graph without walking the whole history.
//!
//! libgit2's sorted revwalks read every reachable commit before they return the first one, so
//! the walk here goes by commit time with a queue of its own and stops after `max` commits. Clock
//! skew can then put a parent before one of its children, which the lane layout can't draw, so
//! the commits read are put back in topological order (children first), keeping time order
//! wherever the history allows it.

use super::CommitInfo;
use crate::error::Result;
use git2::{Oid, Repository};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// The newest commits for the graph, read from `tips`: up to `max` commits, children before
/// parents, and whether more were left out.
///
/// The walk is newest first by commit time and stops after `max` commits. libgit2's revwalk
/// can't do this: any sorting makes it read every reachable commit before returning the first.
pub(super) fn newest_commits(repo: &Repository, tips: &[Oid], max: usize) -> Result<(Vec<CommitInfo>, bool)> {
    let mut queue: BinaryHeap<(i64, Reverse<u64>, Oid)> = BinaryHeap::new();
    let mut seen: HashSet<Oid> = HashSet::new();
    // Ties in time come out in the order they were found, so a tip pushed first comes first.
    let mut order = 0u64;
    let mut push = |queue: &mut BinaryHeap<_>, oid: Oid, time: i64| {
        if seen.insert(oid) {
            order += 1;
            queue.push((time, Reverse(order), oid));
        }
    };
    for &tip in tips {
        let c = repo.find_commit(tip)?;
        push(&mut queue, tip, c.time().seconds());
    }
    let mut commits = Vec::new();
    while let Some((_, _, oid)) = queue.pop() {
        if commits.len() >= max {
            return Ok((children_first(commits), true));
        }
        let c = repo.find_commit(oid)?;
        for p in c.parents() {
            push(&mut queue, p.id(), p.time().seconds());
        }
        commits.push(CommitInfo {
            oid,
            summary: c.summary().unwrap_or_default().to_owned(),
            body: flatten_body(c.body().unwrap_or_default()),
            author: c.author().name().unwrap_or_default().to_owned(),
            email: c.author().email().unwrap_or_default().to_owned(),
            time: c.time().seconds(),
            parents: c.parent_ids().collect(),
        });
    }
    Ok((children_first(commits), false))
}

/// `commits` reordered so every commit comes after all of its children in the list, otherwise
/// keeping the given order (a stable topological sort). Parents outside the list don't count.
pub(super) fn children_first(commits: Vec<CommitInfo>) -> Vec<CommitInfo> {
    let index: HashMap<Oid, usize> = commits.iter().enumerate().map(|(i, c)| (c.oid, i)).collect();
    // Already in order: the common case, so skip the reordering.
    let ordered = commits.iter().enumerate().all(|(i, c)| c.parents.iter().all(|p| index.get(p).is_none_or(|&j| j > i)));
    if ordered {
        return commits;
    }
    let mut waiting = vec![0usize; commits.len()];
    for c in &commits {
        for p in &c.parents {
            if let Some(&j) = index.get(p) {
                waiting[j] += 1;
            }
        }
    }
    let mut ready: BinaryHeap<Reverse<usize>> = (0..commits.len()).filter(|&i| waiting[i] == 0).map(Reverse).collect();
    let mut order = Vec::with_capacity(commits.len());
    while let Some(Reverse(i)) = ready.pop() {
        order.push(i);
        for p in &commits[i].parents {
            if let Some(&j) = index.get(p) {
                waiting[j] -= 1;
                if waiting[j] == 0 {
                    ready.push(Reverse(j));
                }
            }
        }
    }
    let mut slots: Vec<Option<CommitInfo>> = commits.into_iter().map(Some).collect();
    order.into_iter().filter_map(|i| slots[i].take()).collect()
}

pub(super) fn flatten_body(body: &str) -> String {
    let mut out = String::new();
    for word in body.split_whitespace() {
        if out.len() > 240 {
            out.push('…');
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

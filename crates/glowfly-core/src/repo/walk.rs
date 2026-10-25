//! Reading the newest commits for the graph without walking the whole history.
//!
//! libgit2's sorted revwalks read every reachable commit before they return the first one, so
//! the walk here goes by commit time with a queue of its own and stops after `max` commits. Clock
//! skew can then put a parent before one of its children, which the lane layout can't draw, so
//! the commits read are put back in topological order (children first), keeping time order
//! wherever the history allows it.

use super::walk_order::{TimeQueue, walk_parents};
use super::{Authors, CommitInfo};
use crate::error::Result;
use git2::{Oid, Repository};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

/// The newest commits for the graph, read from `tips`: up to `max` commits, children before
/// parents, and whether more were left out.
///
/// The walk is newest first by commit time and stops after `max` commits. libgit2's revwalk
/// can't do this: any sorting makes it read every reachable commit before returning the first.
pub(super) fn newest_commits(repo: &Repository, tips: &[Oid], max: usize) -> Result<(Vec<CommitInfo>, bool)> {
    let mut queue = TimeQueue::default();
    for &tip in tips {
        let c = repo.find_commit(tip)?;
        queue.push(tip, c.time().seconds());
    }
    let mut commits = Vec::new();
    let mut authors = Authors::default();
    let mut body = String::new();
    while let Some(oid) = queue.pop() {
        if commits.len() >= max {
            return Ok((children_first(commits), true));
        }
        let c = repo.find_commit(oid)?;
        for (p, time) in walk_parents(&c) {
            queue.push(p, time);
        }
        // The signature is read once, and the body flattened into a reused buffer.
        let sig = c.author();
        let author = authors.get(sig.name().unwrap_or_default(), sig.email().unwrap_or_default());
        body.clear();
        flatten_body_into(c.body().unwrap_or_default(), &mut body);
        commits.push(CommitInfo {
            oid,
            summary: c.summary().unwrap_or_default().into(),
            body: body.as_str().into(),
            author,
            time: c.time().seconds(),
            parents: c.parent_ids().collect(),
        });
    }
    Ok((children_first(commits), false))
}

/// The texts search matches in commit `c`: its summary and its body flattened to one capped line
/// (`flatten_body`). A message that isn't UTF-8 gives empty texts, as libgit2 gives none.
pub(crate) fn commit_texts(c: &git2::Commit) -> (String, String) {
    (c.summary().unwrap_or_default().to_owned(), flatten_body(c.body().unwrap_or_default()))
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

/// `body` on one line: its words joined by single spaces, cut with "…" after about 240 bytes.
pub(crate) fn flatten_body(body: &str) -> String {
    let mut out = String::new();
    flatten_body_into(body, &mut out);
    out
}

/// `flatten_body`, appended to the empty `out`.
pub(crate) fn flatten_body_into(body: &str, out: &mut String) {
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
}

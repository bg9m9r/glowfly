//! Reading the newest commits for the graph without walking the whole history.
//!
//! libgit2's sorted revwalks read every reachable commit before they return the first one, so
//! the walk here goes by commit time with a queue of its own and stops after `max` commits. Clock
//! skew can then put a parent before one of its children, which the lane layout can't draw, so
//! the commits read are put back in topological order (children first), keeping time order
//! wherever the history allows it.
//!
//! A walk cut short keeps its queue (`Frontier`), so a load for more goes on from it
//! (`more_commits`) instead of walking from the tips again; and a reload takes the commits an
//! earlier load read (`walk_reuse::CommitCache`) instead of reading them again.

use super::walk_order::{TimeQueue, walk_parents};
use super::walk_reuse::{CommitCache, Frontier, authors_of};
use super::{Authors, CommitInfo};
use crate::commit_text::Decoder;
use crate::error::Result;
use git2::{Oid, Repository};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// What a history walk read: the commits, children before parents, whether more were left out,
/// and where it stopped when they were (`Frontier`).
pub(super) struct Walked {
    pub commits: Vec<CommitInfo>,
    pub truncated: bool,
    pub frontier: Option<Frontier>,
}

/// The newest commits for the graph, read from `tips`: up to `max` commits, children before
/// parents, whether more were left out and where the walk stopped. Commits in `cache` are taken
/// from it instead of being read again.
///
/// The walk is newest first by commit time and stops after `max` commits. libgit2's revwalk
/// can't do this: any sorting makes it read every reachable commit before returning the first.
pub(super) fn newest_commits(repo: &Repository, tips: &[Oid], max: usize, cache: Option<&CommitCache>) -> Result<Walked> {
    let mut queue = TimeQueue::default();
    for &tip in tips {
        let time = match cache.and_then(|c| c.get(tip)) {
            Some(c) => c.time,
            None => repo.find_commit(tip)?.time().seconds(),
        };
        queue.push(tip, time);
    }
    let mut authors = cache.map(CommitCache::authors).unwrap_or_default();
    let mut out = Vec::new();
    let reused = walk_into(repo, &mut queue, &mut authors, cache, max, &mut out)?;
    tracing::debug!(reused, read = out.len() - reused, "load.reuse");
    let truncated = !queue.is_empty();
    Ok(Walked { commits: children_first(out), truncated, frontier: truncated.then(|| queue.frontier()) })
}

/// The next `count` commits of a walk that stopped at `frontier` after loading `loaded`; None
/// when one of them is a child of a loaded commit (its clock was behind), which the loaded rows
/// can't take in below them: a full walk puts it above its parent.
pub(super) fn more_commits(repo: &Repository, loaded: &[CommitInfo], frontier: &Frontier, count: usize) -> Result<Option<Walked>> {
    let mut queue = TimeQueue::resume(frontier, loaded.iter().map(|c| c.oid));
    let mut authors = authors_of(loaded);
    let mut out = Vec::new();
    walk_into(repo, &mut queue, &mut authors, None, count, &mut out)?;
    let old: HashSet<Oid> = loaded.iter().map(|c| c.oid).collect();
    if out.iter().any(|c| c.parents.iter().any(|p| old.contains(p))) {
        return Ok(None);
    }
    let truncated = !queue.is_empty();
    Ok(Some(Walked { commits: children_first(out), truncated, frontier: truncated.then(|| queue.frontier()) }))
}

/// Pops commits from `queue` into `out` (in pop order) until it holds `max`, queueing their
/// parents; the commit after the last one stays queued. Commits in `cache` are taken from it.
/// Returns how many were.
fn walk_into(
    repo: &Repository,
    queue: &mut TimeQueue,
    authors: &mut Authors,
    cache: Option<&CommitCache>,
    max: usize,
    out: &mut Vec<CommitInfo>,
) -> Result<usize> {
    let mut body = String::new();
    let mut reused = 0;
    while out.len() < max {
        let Some(oid) = queue.pop() else { break };
        if let Some(c) = cache.and_then(|k| k.get(oid)) {
            // The parents `walk_parents` would give: in order, up to the first that can't be read.
            for &p in &c.parents {
                let Some(time) =
                    cache.and_then(|k| k.get(p)).map(|q| q.time).or_else(|| repo.find_commit(p).ok().map(|q| q.time().seconds()))
                else {
                    break;
                };
                queue.push(p, time);
            }
            out.push(c.clone());
            reused += 1;
            continue;
        }
        let c = repo.find_commit(oid)?;
        for (p, time) in walk_parents(&c) {
            queue.push(p, time);
        }
        // The encoding header and the signature are read once, and the body flattened into a
        // reused buffer; UTF-8 text is borrowed, not copied, on its way in.
        let d = Decoder::of(&c);
        let sig = c.author();
        let author = authors.get(&d.name(&sig), &d.email(&sig));
        body.clear();
        flatten_body_into(&d.body(&c), &mut body);
        out.push(CommitInfo {
            oid,
            summary: d.summary(&c).as_ref().into(),
            body: body.as_str().into(),
            author,
            time: c.time().seconds(),
            parents: c.parent_ids().collect(),
        });
    }
    Ok(reused)
}

/// The texts search matches in commit `c`: its summary and its body flattened to one capped line
/// (`flatten_body`), decoded with the commit's encoding header (`commit_text`).
pub(crate) fn commit_texts(c: &git2::Commit) -> (String, String) {
    let d = Decoder::of(c);
    (d.summary(c).into_owned(), flatten_body(&d.body(c)))
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

//! Whether HEAD contains each local branch's tip (`BranchInfo::merged`), worked out after the
//! graph's walk. Only deleting a branch together with its remote branch asks, so only branches
//! whose upstream is a remote-tracking ref are worked out. Walking history for each tip costs
//! tens of milliseconds on a large repository, and every commit, checkout or pull moves HEAD to
//! a pair the cache hasn't seen, so the answers come, in order, from:
//!
//! - the loaded rows: a tip reached from HEAD through them is contained; when HEAD's whole
//!   history is loaded, a tip not reached isn't;
//! - the cache, for this HEAD;
//! - an earlier HEAD the loaded rows reach from this one: what it contained, this one does,
//!   and of the rest only the commits new since it can be (one walk for every tip);
//! - else a walk for the tip (`GraphCache::contains`).
//!
//! Every answer is kept for this HEAD, so the next load can start from it. The walks go on a
//! handle of their own: once the graph's walk has parsed commits on the load's handle, libgit2
//! reads those commits from disk again on every walk there (its object cache holds them parsed,
//! and walks look for the raw object), which made each walk several times slower.

use super::graph_cache::GraphCache;
use super::{BranchInfo, CommitInfo};
use git2::{Oid, Repository};
use std::collections::{HashMap, HashSet};

/// Most commits a HEAD may have over an earlier one for that one's answers to be used.
const MAX_NEW: usize = 50_000;

/// Sets `merged` on `locals` for `head` (none when HEAD is unborn), the graph's walk having
/// loaded `commits` (at the rows in `row_of`).
pub(super) fn mark(
    repo: &Repository,
    cache: &GraphCache,
    head: Option<Oid>,
    commits: &[CommitInfo],
    row_of: &HashMap<Oid, usize>,
    locals: &mut [BranchInfo],
) {
    let Some(head) = head else { return };
    let asks = |b: &BranchInfo| b.upstream_ref.as_deref().is_some_and(|r| r.starts_with("refs/remotes/"));
    for b in locals.iter_mut() {
        b.merged = b.oid == Some(head);
    }
    let wanted: Vec<usize> = (0..locals.len()).filter(|&i| !locals[i].merged && locals[i].oid.is_some() && asks(&locals[i])).collect();
    if !wanted.is_empty() {
        let loaded = Reach::of(head, commits, row_of);
        let mut earlier = Earlier::new(cache, head, &loaded);
        let own = std::cell::OnceCell::new();
        let walker = || own.get_or_init(|| Repository::open(repo.path()).ok()).as_ref().unwrap_or(repo);
        for i in wanted {
            let Some(tip) = locals[i].oid else { continue };
            let known = if loaded.seen.contains(&tip) {
                Some(true)
            } else if loaded.complete {
                Some(false)
            } else {
                cache.known_contains(repo, head, tip).or_else(|| earlier.answer(repo, cache, tip, walker))
            };
            locals[i].merged = match known {
                Some(yes) => {
                    cache.remember_contains(repo, head, tip, yes);
                    yes
                }
                None => cache.contains(walker(), head, tip),
            };
        }
    }
    cache.note_head(head);
}

/// The loaded commits HEAD reaches through the loaded rows, and whether that is all it reaches
/// (none of them has a parent outside the rows).
struct Reach {
    seen: HashSet<Oid>,
    complete: bool,
}

impl Reach {
    fn of(head: Oid, commits: &[CommitInfo], row_of: &HashMap<Oid, usize>) -> Self {
        let (mut seen, mut complete, mut stack) = (HashSet::new(), true, vec![head]);
        while let Some(o) = stack.pop() {
            let Some(&row) = row_of.get(&o) else {
                complete = false;
                continue;
            };
            if seen.insert(o) {
                stack.extend(commits[row].parents.iter().copied());
            }
        }
        Reach { seen, complete }
    }
}

/// The earlier HEADs this one contains (by the loaded rows), newest first, with the commits
/// this HEAD has over each, read when first needed (None: more than `MAX_NEW`, or unreadable).
struct Earlier {
    head: Oid,
    bases: Vec<Oid>,
    new_since: HashMap<Oid, Option<HashSet<Oid>>>,
}

impl Earlier {
    fn new(cache: &GraphCache, head: Oid, loaded: &Reach) -> Self {
        let bases = cache.recent_heads().into_iter().filter(|h| *h != head && loaded.seen.contains(h)).collect();
        Earlier { head, bases, new_since: HashMap::new() }
    }

    /// Whether HEAD contains `tip`, from the first earlier HEAD with an answer for it; a walk
    /// for the commits new since that one runs on `walker`.
    fn answer<'r>(&mut self, repo: &Repository, cache: &GraphCache, tip: Oid, walker: impl FnOnce() -> &'r Repository) -> Option<bool> {
        let (base, was) = self.bases.iter().find_map(|&b| Some((b, cache.known_contains(repo, b, tip)?)))?;
        if was {
            return Some(true);
        }
        let head = self.head;
        let new = self.new_since.entry(base).or_insert_with(|| new_commits(walker(), head, base));
        new.as_ref().map(|n| n.contains(&tip))
    }
}

/// The commits `head` reaches and `base` doesn't; None past `MAX_NEW` or when the walk fails.
fn new_commits(repo: &Repository, head: Oid, base: Oid) -> Option<HashSet<Oid>> {
    let mut walk = repo.revwalk().ok()?;
    walk.push(head).ok()?;
    walk.hide(base).ok()?;
    let mut out = HashSet::new();
    for oid in walk {
        if out.len() == MAX_NEW {
            return None;
        }
        out.insert(oid.ok()?);
    }
    Some(out)
}

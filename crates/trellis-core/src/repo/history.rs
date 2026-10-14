//! File history: the commits that changed one path, newest first, following renames.
//!
//! The walk follows git's default history simplification: a commit whose file equals one
//! parent's is skipped and only that parent is followed. It examines at most `limit` commits
//! per call and reports where it stopped, so the caller can resume it.

use crate::error::Result;
use git2::{Commit, Delta, DiffFindOptions, ObjectType, Oid, Repository, Tree};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct HistoryEntry {
    pub oid: Oid,
    pub summary: String,
    /// May be empty (the view shows "Unknown").
    pub author: String,
    pub email: String,
    /// Author time (Unix seconds).
    pub time: i64,
    /// The file's path in this commit, `/`-separated.
    pub path: String,
    /// On a rename: the path in the parent.
    pub old_path: Option<String>,
    /// 'A' added, 'M' modified, 'D' deleted, 'R' renamed.
    pub status: char,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileHistory {
    pub entries: Vec<HistoryEntry>,
    /// The walk started at HEAD (rather than at the commit it was asked about).
    pub from_head: bool,
    /// The last commit examined (and its commit time) when the walk stopped early.
    pub searched_until: Option<(Oid, i64)>,
    /// The commits still to examine, with the file's path in each, newest first. Empty when the
    /// walk is complete.
    pub resume: Vec<(Oid, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HistoryStart {
    Head,
    /// From this commit, or from HEAD when HEAD descends from it and still has the path.
    At(Oid),
    /// Continue a walk that stopped early (`FileHistory::resume`).
    Resume(Vec<(Oid, String)>),
}

/// The file at `path` in `tree`: its blob id and mode (a mode change counts as a change).
/// Folders don't count.
fn entry(tree: &Tree<'_>, path: &str) -> Option<(Oid, i32)> {
    tree.get_path(Path::new(path)).ok().filter(|e| e.kind() != Some(ObjectType::Tree)).map(|e| (e.id(), e.filemode()))
}

/// The path `path` (in `tree`) had in `parent_tree`, when the change between them is a rename.
fn renamed_from(repo: &Repository, parent_tree: &Tree<'_>, tree: &Tree<'_>, path: &str) -> Result<Option<String>> {
    let mut diff = repo.diff_tree_to_tree(Some(parent_tree), Some(tree), None)?;
    diff.find_similar(Some(DiffFindOptions::new().renames(true)))?;
    let lossy = |p: Option<&Path>| p.map(|p| p.to_string_lossy().replace('\\', "/"));
    Ok(diff
        .deltas()
        .find(|d| d.status() == Delta::Renamed && lossy(d.new_file().path()).as_deref() == Some(path))
        .and_then(|d| lossy(d.old_file().path())))
}

/// Commits waiting to be examined: newest commit time first and, between equal times, the one
/// queued first (as git orders its walk).
struct Queue {
    heap: BinaryHeap<(i64, Reverse<u64>, Oid)>,
    paths: HashMap<Oid, String>,
    seen: HashSet<Oid>,
    count: u64,
}

impl Queue {
    fn push(&mut self, c: &Commit<'_>, path: &str) {
        let oid = c.id();
        if self.seen.contains(&oid) || self.paths.contains_key(&oid) {
            return; // the first path asked for wins
        }
        self.count += 1;
        self.paths.insert(oid, path.to_owned());
        self.heap.push((c.time().seconds(), Reverse(self.count), oid));
    }

    fn pop(&mut self) -> Option<(Oid, i64, String)> {
        let (time, _, oid) = self.heap.pop()?;
        self.seen.insert(oid);
        Some((oid, time, self.paths.remove(&oid).unwrap_or_default()))
    }

    /// What is left, newest first.
    fn remaining(mut self) -> Vec<(Oid, String)> {
        let mut out = Vec::with_capacity(self.heap.len());
        while let Some((_, _, oid)) = self.heap.pop() {
            out.push((oid, self.paths.remove(&oid).unwrap_or_default()));
        }
        out
    }
}

/// The commits that changed `path`, newest first, examining at most `limit` commits.
pub fn file_history(workdir: &Path, path: &str, start: &HistoryStart, limit: usize) -> Result<FileHistory> {
    let repo = Repository::open(workdir)?;
    let mut out = FileHistory { from_head: true, ..Default::default() };
    let mut queue = Queue { heap: BinaryHeap::new(), paths: HashMap::new(), seen: HashSet::new(), count: 0 };
    let head = || -> Result<Option<Commit<'_>>> {
        match repo.head() {
            Ok(h) => Ok(Some(h.peel_to_commit()?)),
            Err(e) if matches!(e.code(), git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    };
    match start {
        HistoryStart::Head => {
            let Some(h) = head()? else { return Ok(out) };
            queue.push(&h, path);
        }
        HistoryStart::At(c) => {
            let commit = repo.find_commit(*c)?;
            let use_head = match head()? {
                Some(h) => {
                    let below = h.id() == *c || repo.graph_descendant_of(h.id(), *c)?;
                    (below && entry(&h.tree()?, path).is_some()).then_some(h)
                }
                None => None,
            };
            match use_head {
                Some(h) => queue.push(&h, path),
                None => {
                    out.from_head = false;
                    queue.push(&commit, path);
                }
            }
        }
        HistoryStart::Resume(v) => {
            for (oid, p) in v {
                queue.push(&repo.find_commit(*oid)?, p);
            }
        }
    }

    let mut last = None;
    for _ in 0..limit.max(1) {
        let Some((oid, time, p)) = queue.pop() else { break };
        last = Some((oid, time));
        examine(&repo, oid, &p, &mut queue, &mut out.entries)?;
    }
    if !queue.heap.is_empty() {
        out.searched_until = last;
        out.resume = queue.remaining();
    }
    Ok(out)
}

/// Looks at one commit: records an entry if it changed the file at `p`, and queues the parents
/// to follow.
fn examine(repo: &Repository, oid: Oid, p: &str, queue: &mut Queue, entries: &mut Vec<HistoryEntry>) -> Result<()> {
    let c = repo.find_commit(oid)?;
    let tree = c.tree()?;
    let me = entry(&tree, p);
    let parents: Vec<Commit<'_>> = c.parents().collect();
    let trees: Vec<Tree<'_>> = parents.iter().map(|q| q.tree()).collect::<std::result::Result<_, _>>()?;
    let theirs: Vec<Option<(Oid, i32)>> = trees.iter().map(|t| entry(t, p)).collect();
    let make = |status: char, old_path: Option<String>| HistoryEntry {
        oid,
        summary: String::from_utf8_lossy(c.summary_bytes().unwrap_or_default()).into_owned(),
        author: String::from_utf8_lossy(c.author().name_bytes()).into_owned(),
        email: String::from_utf8_lossy(c.author().email_bytes()).into_owned(),
        time: c.author().when().seconds(),
        path: p.to_owned(),
        old_path,
        status,
    };

    if parents.is_empty() {
        if me.is_some() {
            entries.push(make('A', None));
        }
        return Ok(());
    }
    if me.is_some()
        && let Some(i) = theirs.iter().position(|t| *t == me)
    {
        // Unchanged against this parent: the file's history is that parent's.
        queue.push(&parents[i], p);
        return Ok(());
    }
    if me.is_none() {
        // Only a start commit can lack the path: it deleted the file, or never had it.
        if theirs.iter().any(Option::is_some) {
            entries.push(make('D', None));
            for (q, t) in parents.iter().zip(&theirs) {
                if t.is_some() {
                    queue.push(q, p);
                }
            }
        }
        return Ok(());
    }
    // Changed against every parent.
    let mut status = None;
    let mut first_lacking = true;
    let mut any_rename = false;
    for ((q, qt), t) in parents.iter().zip(&trees).zip(&theirs) {
        if t.is_some() {
            queue.push(q, p);
            continue;
        }
        let old = renamed_from(repo, qt, &tree, p)?;
        if let Some(old) = &old {
            queue.push(q, old);
            any_rename = true;
        }
        if first_lacking {
            first_lacking = false;
            status = old.map(|o| ('R', Some(o)));
        }
    }
    let (status, old_path) = status.unwrap_or_else(|| {
        let created = !any_rename && theirs.iter().all(Option::is_none);
        (if created { 'A' } else { 'M' }, None)
    });
    entries.push(make(status, old_path));
    Ok(())
}

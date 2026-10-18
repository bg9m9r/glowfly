//! File history: the commits that changed one path, newest first, following renames.
//!
//! The walk follows git's default history simplification: a commit whose file equals one
//! parent's is skipped and only that parent is followed. It examines at most `limit` commits
//! per call and reports where it stopped, so the caller can resume it. Each call also reports
//! whether the path is on disk in the working tree. A walk asked about an older commit whose
//! file was renamed since starts at HEAD under the new name (`history_forward.rs`). The same
//! walk lists a folder's commits (`folder_history.rs`).

use super::FileEntry;
use crate::error::Result;
use git2::{Commit, Delta, Diff, DiffFindOptions, ObjectType, Oid, Repository, Tree};
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
    /// The file's path in this commit, `/`-separated (the folder, in a folder history).
    pub path: String,
    /// On a rename: the path in the parent (a folder's former name, in a folder history).
    pub old_path: Option<String>,
    /// 'A' added, 'M' modified, 'D' deleted, 'R' renamed.
    pub status: char,
    /// A folder history's changed files under the folder (full repo paths, vs the first
    /// parent); empty in a file history.
    pub files: Vec<FileEntry>,
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
    /// Whether something is at the file's name at HEAD (`head_path`, else the path the walk was
    /// asked about) in the working tree; false in a bare repository.
    pub on_disk: bool,
    /// Set when the walk followed renames forwards: the file's name at HEAD, which differs from
    /// the path asked about.
    pub head_path: Option<String>,
    /// A folder history's files, by their names today (`folder_file_today` against the folder
    /// asked about), with nothing at them in the working tree (and every name that isn't
    /// UTF-8); empty in a file history.
    pub absent: HashSet<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HistoryStart {
    Head,
    /// From HEAD when HEAD is or descends from this commit and has the path; else from HEAD
    /// under the file's later name when renames lead there; else from this commit.
    At(Oid),
    /// From HEAD, for a history that was at HEAD when HEAD was this commit. When HEAD descends
    /// from it and lost the path, follow renames forwards; otherwise plain HEAD.
    Moved(Oid),
    /// Continue a walk that stopped early (`FileHistory::resume`).
    Resume(Vec<(Oid, String)>),
}

/// What a walk follows: one file, or everything under a folder.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    File,
    Folder,
}

/// What is at `path` in `tree`: its id and mode (a mode change counts as a change). A file
/// history takes anything but a folder; a folder history only a folder.
pub(super) fn present(tree: &Tree<'_>, path: &str, kind: Kind) -> Option<(Oid, i32)> {
    let wanted = |e: &git2::TreeEntry<'_>| (e.kind() == Some(ObjectType::Tree)) == (kind == Kind::Folder);
    tree.get_path(Path::new(path)).ok().filter(wanted).map(|e| (e.id(), e.filemode()))
}

/// The diff from `parent_tree` to `tree`, with renames detected.
pub(super) fn renames<'r>(repo: &'r Repository, parent_tree: &Tree<'_>, tree: &Tree<'_>) -> Result<Diff<'r>> {
    let mut diff = repo.diff_tree_to_tree(Some(parent_tree), Some(tree), None)?;
    diff.find_similar(Some(DiffFindOptions::new().renames(true)))?;
    Ok(diff)
}

/// The path `path` (in `tree`) had in `parent_tree`, when the change between them is a rename.
fn renamed_from(repo: &Repository, parent_tree: &Tree<'_>, tree: &Tree<'_>, path: &str) -> Result<Option<String>> {
    // Byte for byte: a '\' is part of a name on Unix, not a separator. A source whose name
    // isn't UTF-8 ends the walk there, as if the file had been added.
    Ok(renames(repo, parent_tree, tree)?
        .deltas()
        .find(|d| d.status() == Delta::Renamed && super::rpath::is(&d.new_file(), path))
        .and_then(|d| d.old_file().path_bytes().and_then(super::rpath::text)))
}

/// The path `old` (in `parent_tree`) has in `tree`, when the change between them is a rename:
/// `renamed_from` the other way round.
pub(super) fn renamed_to(repo: &Repository, parent_tree: &Tree<'_>, tree: &Tree<'_>, old: &str) -> Result<Option<String>> {
    Ok(renames(repo, parent_tree, tree)?
        .deltas()
        .find(|d| d.status() == Delta::Renamed && super::rpath::is(&d.old_file(), old))
        .and_then(|d| d.new_file().path_bytes().and_then(super::rpath::text)))
}

/// Commits waiting to be examined: newest commit time first and, between equal times, the one
/// queued first (as git orders its walk).
pub(super) struct Queue {
    heap: BinaryHeap<(i64, Reverse<u64>, Oid)>,
    paths: HashMap<Oid, String>,
    seen: HashSet<Oid>,
    count: u64,
}

impl Queue {
    fn new() -> Self {
        Self { heap: BinaryHeap::new(), paths: HashMap::new(), seen: HashSet::new(), count: 0 }
    }

    pub(super) fn push(&mut self, c: &Commit<'_>, path: &str) {
        let oid = c.id();
        if self.seen.contains(&oid) || self.paths.contains_key(&oid) {
            return; // the first path asked for wins
        }
        self.count += 1;
        self.paths.insert(oid, path.to_owned());
        self.heap.push((c.time().seconds(), Reverse(self.count), oid));
    }

    pub(super) fn pop(&mut self) -> Option<(Oid, i64, String)> {
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
    walk(workdir, path, start, limit, Kind::File)
}

/// HEAD's commit; `None` on an unborn branch.
pub(super) fn head(repo: &Repository) -> Result<Option<Commit<'_>>> {
    match repo.head() {
        Ok(h) => Ok(Some(h.peel_to_commit()?)),
        Err(e) if matches!(e.code(), git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// The history of the file or folder (`kind`) at `path`, examining at most `limit` commits.
pub(super) fn walk(workdir: &Path, path: &str, start: &HistoryStart, limit: usize, kind: Kind) -> Result<FileHistory> {
    let repo = Repository::open(workdir)?;
    let mut out = FileHistory { from_head: true, ..Default::default() };
    let mut queue = Queue::new();
    match start {
        HistoryStart::Head => {
            let Some(h) = head(&repo)? else { return Ok(out) };
            queue.push(&h, path);
        }
        HistoryStart::At(c) | HistoryStart::Moved(c) => {
            let moved = matches!(start, HistoryStart::Moved(_));
            match head(&repo)? {
                None if moved => return Ok(out),
                None => {
                    out.from_head = false;
                    queue.push(&repo.find_commit(*c)?, path);
                }
                Some(h) => {
                    // A commit gone since (a Moved start after an amend, say) is not below HEAD.
                    let below = h.id() == *c
                        || match repo.graph_descendant_of(h.id(), *c) {
                            Ok(b) => b,
                            Err(_) if moved => false,
                            Err(e) => return Err(e.into()),
                        };
                    let at_head = below && present(&h.tree()?, path, kind).is_some();
                    let forward = if !at_head && below && kind == Kind::File {
                        super::history_forward::name_at(&repo, *c, h.id(), path)?
                    } else {
                        None
                    };
                    if let Some(n) = forward {
                        queue.push(&h, &n);
                        out.head_path = Some(n);
                    } else if at_head || moved {
                        queue.push(&h, path);
                    } else {
                        out.from_head = false;
                        queue.push(&repo.find_commit(*c)?, path);
                    }
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
        match kind {
            Kind::File => examine(&repo, oid, &p, &mut queue, &mut out.entries)?,
            Kind::Folder => super::folder_history::examine(&repo, oid, &p, &mut queue, &mut out.entries)?,
        }
    }
    if !queue.heap.is_empty() {
        out.searched_until = last;
        out.resume = queue.remaining();
    }
    out.on_disk = super::diff::on_disk(&repo, out.head_path.as_deref().unwrap_or(path));
    if kind == Kind::Folder {
        for e in &out.entries {
            for f in &e.files {
                let today = super::folder_history::folder_file_today(&e.path, path, &f.path);
                if !out.absent.contains(&today) && (f.hazard.is_some() || !super::diff::on_disk(&repo, &today)) {
                    out.absent.insert(today);
                }
            }
        }
    }
    Ok(out)
}

/// The entry for commit `c` with the file (or folder) at `path`.
pub(super) fn entry_of(c: &Commit<'_>, path: &str, old_path: Option<String>, status: char, files: Vec<FileEntry>) -> HistoryEntry {
    HistoryEntry {
        oid: c.id(),
        summary: String::from_utf8_lossy(c.summary_bytes().unwrap_or_default()).into_owned(),
        author: String::from_utf8_lossy(c.author().name_bytes()).into_owned(),
        email: String::from_utf8_lossy(c.author().email_bytes()).into_owned(),
        time: c.author().when().seconds(),
        path: path.to_owned(),
        old_path,
        status,
        files,
    }
}

/// Looks at one commit: records an entry if it changed the file at `p`, and queues the parents
/// to follow.
fn examine(repo: &Repository, oid: Oid, p: &str, queue: &mut Queue, entries: &mut Vec<HistoryEntry>) -> Result<()> {
    let c = repo.find_commit(oid)?;
    let tree = c.tree()?;
    let me = present(&tree, p, Kind::File);
    let parents: Vec<Commit<'_>> = c.parents().collect();
    let trees: Vec<Tree<'_>> = parents.iter().map(|q| q.tree()).collect::<std::result::Result<_, _>>()?;
    let theirs: Vec<Option<(Oid, i32)>> = trees.iter().map(|t| present(t, p, Kind::File)).collect();
    let make = |status: char, old_path: Option<String>| entry_of(&c, p, old_path, status, Vec::new());

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

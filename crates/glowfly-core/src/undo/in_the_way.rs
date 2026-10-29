//! Files an undo or redo would write over without a word: git keeps no copy of an ignored file,
//! and `reset --hard` replaces untracked ones too.
//!
//! - Moving HEAD (`reset --keep`, `reset --hard`, a checkout) writes the paths the target tracks
//!   and HEAD doesn't (`check`).
//! - Putting a stash back (`stash pop`, `apply`) writes the paths its trees hold that the commit
//!   it goes onto doesn't track (`check_stash`).
//!
//! Something on disk is in the way of such a path when it is a file nothing tracks (untracked or
//! ignored; matched without case when `core.ignorecase` is set, so a case-only rename isn't), a
//! folder holding one, or a file nothing tracks where one of its folders goes. A submodule's
//! folder is left alone: checking one out writes nothing into it. Anything in the way refuses
//! the plan, naming the files, and the entry stays for when they have been moved.

use super::{Action, Plan, Refusal};
use crate::error::Result;
use git2::{Delta, FileMode, Oid, Repository, Tree, TreeWalkMode, TreeWalkResult};
use std::collections::HashSet;
use std::path::Path;

/// How many paths a refusal names before "and N more".
const NAMED: usize = 3;

/// The paths HEAD and the index track, folded to lower case when the file system ignores case.
struct Tracked {
    paths: HashSet<String>,
    fold: bool,
}

impl Tracked {
    fn of(repo: &Repository) -> Result<Self> {
        let fold = repo.config().ok().and_then(|c| c.get_bool("core.ignorecase").ok()).unwrap_or(false);
        let mut t = Tracked { paths: HashSet::new(), fold };
        if let Some(head) = repo.head().ok().and_then(|h| h.peel_to_tree().ok()) {
            for (path, _) in blobs(&head)? {
                t.add(&path);
            }
        }
        for e in repo.index()?.iter() {
            t.add(&String::from_utf8_lossy(&e.path));
        }
        Ok(t)
    }

    fn key(&self, p: &str) -> String {
        if self.fold { p.to_lowercase() } else { p.to_owned() }
    }

    fn add(&mut self, p: &str) {
        let k = self.key(p);
        self.paths.insert(k);
    }

    fn has(&self, p: &str) -> bool {
        self.paths.contains(&self.key(p))
    }
}

/// Checks the paths of one plan against the disk, remembering the folders already looked at.
struct Disk<'a> {
    workdir: &'a Path,
    tracked: Tracked,
    folders: HashSet<String>,
}

impl Disk<'_> {
    /// Whether something nothing tracks is in the way of writing `path` (a gitlink with `link`).
    fn blocks(&mut self, path: &str, link: bool) -> bool {
        if let Ok(m) = self.workdir.join(path).symlink_metadata() {
            let blocked = if m.is_dir() { !link && self.holds_untracked(path) } else { !self.tracked.has(path) };
            if blocked {
                return true;
            }
        }
        for (i, _) in path.match_indices('/') {
            let folder = &path[..i];
            if !self.folders.insert(folder.to_owned()) {
                continue;
            }
            let file_there = self.workdir.join(folder).symlink_metadata().is_ok_and(|m| !m.is_dir());
            if file_there && !self.tracked.has(folder) {
                return true;
            }
        }
        false
    }

    /// Whether folder `rel` holds a file nothing tracks, at any depth.
    fn holds_untracked(&self, rel: &str) -> bool {
        let Ok(entries) = std::fs::read_dir(self.workdir.join(rel)) else { return false };
        entries.flatten().any(|e| {
            let child = format!("{rel}/{}", e.file_name().to_string_lossy());
            match e.file_type() {
                Ok(t) if t.is_dir() => self.holds_untracked(&child),
                _ => !self.tracked.has(&child),
            }
        })
    }
}

/// Every blob and gitlink in `tree`, with whether it is a gitlink.
fn blobs(tree: &Tree) -> Result<Vec<(String, bool)>> {
    let mut out = Vec::new();
    tree.walk(TreeWalkMode::PreOrder, |dir, e| {
        let mode = e.filemode();
        if mode != i32::from(FileMode::Tree) {
            out.push((format!("{dir}{}", String::from_utf8_lossy(e.name_bytes())), mode == i32::from(FileMode::Commit)));
        }
        TreeWalkResult::Ok
    })?;
    Ok(out)
}

fn blocked(workdir: &Path, repo: &Repository, wanted: Vec<(String, bool)>) -> Result<Vec<String>> {
    let mut disk = Disk { workdir, tracked: Tracked::of(repo)?, folders: HashSet::new() };
    Ok(wanted.into_iter().filter(|(p, link)| disk.blocks(p, *link)).map(|(p, _)| p).collect())
}

/// The paths `target` tracks that HEAD doesn't, which something on disk is in the way of.
pub(super) fn in_the_way(workdir: &Path, repo: &Repository, target: Oid) -> Result<Vec<String>> {
    let to = repo.find_commit(target)?.tree()?;
    let head = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let diff = repo.diff_tree_to_tree(head.as_ref(), Some(&to), None)?;
    let added = diff
        .deltas()
        .filter(|d| d.status() == Delta::Added)
        .filter_map(|d| {
            let path = String::from_utf8_lossy(d.new_file().path_bytes()?).into_owned();
            Some((path, d.new_file().mode() == FileMode::Commit))
        })
        .collect();
    blocked(workdir, repo, added)
}

/// The paths stash `stash` holds (its working tree, index and untracked files) that `onto`
/// doesn't track, which something on disk is in the way of.
pub(super) fn stash_in_the_way(workdir: &Path, repo: &Repository, stash: Oid, onto: Oid) -> Result<Vec<String>> {
    let s = repo.find_commit(stash)?;
    let base = repo.find_commit(onto)?.tree()?;
    let mut trees = vec![s.tree()?];
    for n in 1..s.parent_count() {
        trees.push(s.parent(n)?.tree()?);
    }
    let mut seen = HashSet::new();
    let mut wanted = Vec::new();
    for t in &trees {
        for (path, link) in blobs(t)? {
            if base.get_path(Path::new(&path)).is_err() && seen.insert(path.clone()) {
                wanted.push((path, link));
            }
        }
    }
    blocked(workdir, repo, wanted)
}

/// The refusal for `paths` in the way of undoing (or redoing) `action`: `Unsafe`, since stashing
/// can't move ignored files out of the way, so no stash is offered.
pub(super) fn refusal(paths: &[String], action: &Action, redo: bool) -> Plan {
    let verb = if redo { "redoing" } else { "undoing" };
    let mut names = paths.iter().take(NAMED).cloned().collect::<Vec<_>>().join(", ");
    if paths.len() > NAMED {
        names.push_str(&format!(" and {} more", paths.len() - NAMED));
    }
    Plan::Refuse(Refusal::Unsafe(format!(
        "Move {names} away first: {verb} {} would write over {}, which git doesn't keep a copy of.",
        action.describe(),
        if paths.len() == 1 { "it" } else { "them" }
    )))
}

/// `refusal` when something is in the way of moving to `target`.
pub(super) fn check(workdir: &Path, repo: &Repository, target: Oid, action: &Action, redo: bool) -> Result<Option<Plan>> {
    let paths = in_the_way(workdir, repo, target)?;
    Ok((!paths.is_empty()).then(|| refusal(&paths, action, redo)))
}

/// `refusal` when something is in the way of putting stash `stash` back onto commit `onto`.
pub(super) fn check_stash(workdir: &Path, repo: &Repository, stash: Oid, onto: Oid, action: &Action, redo: bool) -> Result<Option<Plan>> {
    let paths = stash_in_the_way(workdir, repo, stash, onto)?;
    Ok((!paths.is_empty()).then(|| refusal(&paths, action, redo)))
}

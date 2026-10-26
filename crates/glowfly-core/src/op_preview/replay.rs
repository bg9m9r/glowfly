//! The in-memory replay engine behind the rebase, cherry-pick and drop previews: picks and
//! recreated merges applied with libgit2's three-way merge on top of a moving tip, the way
//! git's sequencer applies them.
//!
//! Nothing reaches the disk: `preview_with` added a mempack backend to the object database
//! with the highest priority, so the trees and commits written here stay in memory and are
//! freed with the `Repository`. Commits are made with no ref to update, and no index or
//! working file is touched.

use crate::commit_text;
use crate::error::{Error, Result};
use crate::rebase::{MergeParent, ResetTo};
use git2::{Commit, Index, MergeOptions, Oid, Repository, Tree};
use std::collections::HashMap;

/// A replay in progress.
pub(super) struct Replay<'r> {
    repo: &'r Repository,
    opts: MergeOptions,
    /// Where the next step applies; None is an empty new root.
    pub tip: Option<Oid>,
    /// Each commit made in memory, with the original it replays.
    pub made: HashMap<Oid, Oid>,
    labels: HashMap<String, Option<Oid>>,
    onto: Option<Oid>,
    empty_tree: Oid,
}

/// What one step did.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Applied {
    /// A new commit (in memory).
    New(Oid),
    /// The original commit is used as it is (its parent is where the tip stands).
    Kept,
    /// The commit's changes are already there.
    Emptied,
    /// It stops with conflicts in these paths.
    Conflicts(Vec<String>),
}

impl<'r> Replay<'r> {
    /// A replay starting at `onto` (None: a new root).
    pub fn new(repo: &'r Repository, onto: Option<Oid>) -> Result<Self> {
        let empty_tree = repo.treebuilder(None)?.write()?;
        Ok(Self { repo, opts: MergeOptions::new(), tip: onto, made: HashMap::new(), labels: HashMap::new(), onto, empty_tree })
    }

    /// Applies commit `c` on the tip. With `ff`, a commit whose parent is the tip is kept as
    /// it is (the sequencer's fast-forward). A commit that changes nothing to begin with is
    /// committed again with `keep_started_empty` (a rebase keeps it) and is `Emptied`
    /// otherwise (a cherry-pick refuses it).
    pub fn pick(&mut self, c: &Commit<'_>, ff: bool, keep_started_empty: bool) -> Result<Applied> {
        let parent = c.parent_ids().next();
        if ff && self.tip == parent {
            self.tip = Some(c.id());
            return Ok(Applied::Kept);
        }
        let base = match parent {
            Some(p) => self.repo.find_commit(p)?.tree()?,
            None => self.tree(self.empty_tree)?,
        };
        let ours_id = self.tip_tree()?;
        if c.tree_id() == base.id() {
            if !keep_started_empty {
                return Ok(Applied::Emptied);
            }
            return self.commit(c, ours_id, &[]).map(Applied::New);
        }
        let ours = self.tree(ours_id)?;
        let mut index = self.repo.merge_trees(&base, &ours, &c.tree()?, Some(&self.opts))?;
        if index.has_conflicts() {
            return Ok(Applied::Conflicts(conflict_paths(&index)?));
        }
        let tree = index.write_tree_to(self.repo)?;
        if tree == ours_id {
            return Ok(Applied::Emptied);
        }
        self.commit(c, tree, &[]).map(Applied::New)
    }

    /// Recreates the merge `original` with `other` as its second parent (`merge -C`). It is
    /// kept as it is when both its parents are where they were.
    pub fn merge_step(&mut self, original: &Commit<'_>, other: Oid) -> Result<Applied> {
        let parents: Vec<Oid> = original.parent_ids().collect();
        if self.tip == parents.first().copied() && parents.get(1) == Some(&other) {
            self.tip = Some(original.id());
            return Ok(Applied::Kept);
        }
        let Some(tip) = self.tip else {
            return Err(Error::Other("a recreated merge has no first parent".into()));
        };
        let (ours, theirs) = (self.repo.find_commit(tip)?, self.repo.find_commit(other)?);
        let mut index = self.repo.merge_commits(&ours, &theirs, Some(&self.opts))?;
        if index.has_conflicts() {
            return Ok(Applied::Conflicts(conflict_paths(&index)?));
        }
        let tree = index.write_tree_to(self.repo)?;
        self.commit(original, tree, &[other]).map(Applied::New)
    }

    /// `label <name>`: names where the tip stands.
    pub fn label(&mut self, name: &str) {
        self.labels.insert(name.to_owned(), self.tip);
    }

    /// `reset <to>`: moves the tip.
    pub fn reset(&mut self, to: &ResetTo) -> Result<()> {
        self.tip = match to {
            ResetTo::Onto => self.onto,
            ResetTo::Label(name) => self.labelled(name)?,
            ResetTo::Commit(o) => Some(*o),
            ResetTo::NewRoot => None,
        };
        Ok(())
    }

    /// The commit a merge's later parent names.
    pub fn parent(&self, p: &MergeParent) -> Result<Oid> {
        match p {
            MergeParent::Commit(o) => Ok(*o),
            MergeParent::Label(name) => self.labelled(name)?.ok_or_else(|| Error::Other(format!("label {name} names no commit"))),
        }
    }

    fn labelled(&self, name: &str) -> Result<Option<Oid>> {
        self.labels.get(name).copied().ok_or_else(|| Error::Other(format!("no label {name}")))
    }

    fn tree(&self, id: Oid) -> Result<Tree<'r>> {
        Ok(self.repo.find_tree(id)?)
    }

    fn tip_tree(&self) -> Result<Oid> {
        match self.tip {
            Some(t) => Ok(self.repo.find_commit(t)?.tree_id()),
            None => Ok(self.empty_tree),
        }
    }

    /// Commits `tree` on the tip (plus `extra` parents) with `like`'s author, committer and
    /// message, in memory, and moves the tip to it.
    fn commit(&mut self, like: &Commit<'_>, tree: Oid, extra: &[Oid]) -> Result<Oid> {
        let tree = self.tree(tree)?;
        let mut parents: Vec<Commit<'_>> = Vec::new();
        if let Some(t) = self.tip {
            parents.push(self.repo.find_commit(t)?);
        }
        for o in extra {
            parents.push(self.repo.find_commit(*o)?);
        }
        let refs: Vec<&Commit<'_>> = parents.iter().collect();
        // Written without an encoding header, so the preview's text must be UTF-8 to read back.
        let message = commit_text::message(like);
        let new = self.repo.commit(None, &like.author(), &like.committer(), &message, &tree, &refs)?;
        self.made.insert(new, like.id());
        self.tip = Some(new);
        Ok(new)
    }
}

/// The paths of `index`'s conflicts, in index order, each once, for the screen.
fn conflict_paths(index: &Index) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for c in index.conflicts()? {
        let c = c?;
        let entry = c.our.as_ref().or(c.their.as_ref()).or(c.ancestor.as_ref());
        if let Some(e) = entry {
            let shown = crate::repo::shown_path(&e.path);
            if !out.contains(&shown) {
                out.push(shown);
            }
        }
    }
    Ok(out)
}

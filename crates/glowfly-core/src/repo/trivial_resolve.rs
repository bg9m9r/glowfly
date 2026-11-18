//! Taking the trivial picks in every conflicted file at once: `trivial_plan` reads the conflicts
//! and their working files and sorts each path into resolved (text, every chunk has a trivial
//! pick, the working copy is as git wrote it), left (some chunk needs a decision) or skipped
//! (with the reason); `TrivialPlan::apply` then resolves the first kind through
//! `write_resolution`, which keeps git's resolve-undo record.

use super::conflict::{ConflictContent, ConflictFile, ConflictIds, load_conflict};
use super::conflict_saved::{WorkingCopy, working_copy};
use super::resolve::{Choice, render_output, write_resolution};
use crate::conflict_rows::trivial_picks;
use crate::error::{ReadFailed, Result};
use std::path::Path;

/// Why a conflicted file was not looked at for trivial picks.
#[derive(Clone, Debug, PartialEq)]
pub enum TrivialSkip {
    Binary,
    /// Deleted on one side, a symbolic link, a submodule or not UTF-8.
    NotText,
    /// Git converts the file (the attribute that says so).
    Converted(String),
    /// The working copy already holds saved picks or text typed in.
    SavedPicks,
    /// The working copy's markers can't be matched with the conflict.
    Edited,
    /// The working file can't be read as text.
    Unreadable,
    /// Reading the conflict failed.
    Failed(ReadFailed),
}

/// What `TrivialPlan::apply` resolved and what it didn't, by path.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrivialTaken {
    pub resolved: Vec<String>,
    /// Text conflicts with a chunk that has no trivial pick (or no chunk at all).
    pub left: Vec<String>,
    pub skipped: Vec<(String, TrivialSkip)>,
}

struct Planned {
    path: String,
    contents: String,
    ids: ConflictIds,
}

/// The files to resolve, in input order, with the ones left and skipped.
pub struct TrivialPlan {
    resolve: Vec<Planned>,
    pub left: Vec<String>,
    pub skipped: Vec<(String, TrivialSkip)>,
}

impl TrivialPlan {
    /// The paths this plan resolves.
    pub fn paths(&self) -> Vec<String> {
        self.resolve.iter().map(|p| p.path.clone()).collect()
    }

    /// Resolves each planned file in order, stopping at the first error.
    pub fn apply(&self, workdir: &Path) -> Result<()> {
        for p in &self.resolve {
            write_resolution(workdir, &p.path, &p.contents, &p.ids)?;
        }
        Ok(())
    }

    pub fn into_taken(self) -> TrivialTaken {
        TrivialTaken { resolved: self.paths(), left: self.left, skipped: self.skipped }
    }
}

/// What to do with one loaded conflict.
enum Decided {
    Resolve(String),
    Left,
    Skip(TrivialSkip),
}

/// Sorts `paths` (conflicted, repo-relative). Only reads. A path no longer conflicted is dropped.
pub fn trivial_plan(workdir: &Path, paths: &[String]) -> TrivialPlan {
    let mut plan = TrivialPlan { resolve: Vec::new(), left: Vec::new(), skipped: Vec::new() };
    for path in paths {
        let file = match load_conflict(workdir, path) {
            Ok(f) => f,
            Err(ReadFailed::NotConflicted { .. }) => continue,
            Err(e) => {
                plan.skipped.push((path.clone(), TrivialSkip::Failed(e)));
                continue;
            }
        };
        match decide(&file, working_copy(workdir, &file)) {
            Decided::Resolve(contents) => plan.resolve.push(Planned { path: path.clone(), contents, ids: file.ids.clone() }),
            Decided::Left => plan.left.push(path.clone()),
            Decided::Skip(why) => plan.skipped.push((path.clone(), why)),
        }
    }
    plan
}

fn decide(file: &ConflictFile, working: WorkingCopy) -> Decided {
    match &file.content {
        ConflictContent::Binary => return Decided::Skip(TrivialSkip::Binary),
        ConflictContent::DeletedBy(_) | ConflictContent::Unsupported(_) => return Decided::Skip(TrivialSkip::NotText),
        ConflictContent::Text(_) => {}
    }
    match working {
        WorkingCopy::Converted(a) => return Decided::Skip(TrivialSkip::Converted(a)),
        WorkingCopy::Unreadable => return Decided::Skip(TrivialSkip::Unreadable),
        WorkingCopy::Saved(_) => return Decided::Skip(TrivialSkip::SavedPicks),
        WorkingCopy::Unmatched => return Decided::Skip(TrivialSkip::Edited),
        WorkingCopy::Untouched => {}
    }
    if file.chunk_count() == 0 {
        return Decided::Left;
    }
    let choices: Option<Vec<Choice>> = trivial_picks(file).into_iter().map(|p| p.map(|t| t.0)).collect();
    match choices.and_then(|c| render_output(file, &c)) {
        Some(contents) => Decided::Resolve(contents),
        None => Decided::Left,
    }
}

#[cfg(test)]
#[path = "trivial_resolve_tests.rs"]
mod tests;

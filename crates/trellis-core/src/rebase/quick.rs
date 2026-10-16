//! History edits chosen in the graph (reword, squash, drop), turned into a ready
//! [`StartRequest`] for the same `git rebase -i` run the panel uses.

use super::plan::{checked_head, merge_refused, plan};
use super::todo::problem;
use super::{
    DROPS_EVERYTHING, MERGE_SELECTED, NOT_CONSECUTIVE, NOT_ON_BRANCH, PARENT_MERGE, ROOT_SQUASH, RebaseAction, RebaseBase,
    SOME_MERGE_SELECTED, SOME_NOT_ON_BRANCH, StartRequest, TOO_FEW_TO_SQUASH, Upstream,
};
use crate::error::{Error, Result};
use git2::{Oid, Repository};
use std::collections::HashSet;
use std::path::Path;

/// A history edit on the current branch.
#[derive(Clone, PartialEq, Debug)]
pub enum QuickEdit {
    /// Give the commit a new message (typed afterwards, into the plan's row).
    Reword(Oid),
    /// Meld the commit into its parent: the parent's place and author, both messages.
    SquashIntoParent(Oid),
    /// Meld consecutive commits into the oldest of them, with all their messages.
    Squash(Vec<Oid>),
    /// Remove the commits and their changes.
    Drop(Vec<Oid>),
}

impl QuickEdit {
    /// The commits chosen, without repeats.
    fn targets(&self) -> Vec<Oid> {
        let list = match self {
            Self::Reword(o) | Self::SquashIntoParent(o) => vec![*o],
            Self::Squash(v) | Self::Drop(v) => v.clone(),
        };
        let mut seen = HashSet::new();
        list.into_iter().filter(|o| seen.insert(*o)).collect()
    }
}

/// An edit ready to run.
#[derive(Clone, Debug)]
pub struct QuickPlan {
    pub edit: QuickEdit,
    /// The list to run; `autostash` is false (the caller decides after asking).
    pub request: StartRequest,
    pub branch: String,
    /// The rows of `request.items` the edit changes, oldest first.
    pub edited: Vec<usize>,
    /// Commits after the last edited row, which are rewritten too.
    pub later: usize,
}

/// The rebase that makes `edit` on the current branch. Refused (`Error::Unsupported`, with a
/// message for the user) while an operation is in progress, on a detached HEAD, for commits
/// that aren't on the branch's first-parent line, for merges (chosen, or between the commits
/// and HEAD), for a squash of the first commit or into a merge, for a squash of commits that
/// don't follow one another, and for dropping every commit of the branch.
pub fn prepare_edit(workdir: &Path, edit: &QuickEdit) -> Result<QuickPlan> {
    let repo = Repository::open(workdir)?;
    let (head, _) = checked_head(&repo)?;
    let targets = edit.targets();
    let many = targets.len() > 1;
    if matches!(edit, QuickEdit::Squash(_)) && !many {
        return Err(refused(TOO_FEW_TO_SQUASH));
    }
    let oldest = oldest_on_branch(&repo, head, &targets, many)?;
    let base = match edit {
        QuickEdit::SquashIntoParent(o) => {
            let commit = repo.find_commit(*o)?;
            let parent = commit.parent_ids().next().ok_or_else(|| refused(ROOT_SQUASH))?;
            if repo.find_commit(parent)?.parent_count() > 1 {
                return Err(refused(PARENT_MERGE));
            }
            parent
        }
        _ => oldest,
    };
    let p = plan(workdir, &RebaseBase::FromCommit(base))?;
    let mut items = p.items;
    let row_of = |o: Oid| items.iter().position(|i| i.oid == o);
    let mut rows: Vec<usize> = targets.iter().filter_map(|o| row_of(*o)).collect();
    if let QuickEdit::SquashIntoParent(_) = edit {
        rows.extend(row_of(base));
    }
    rows.sort_unstable();
    rows.dedup();
    if rows.is_empty() {
        return Err(refused(if many { SOME_NOT_ON_BRANCH } else { NOT_ON_BRANCH }));
    }
    match edit {
        QuickEdit::Reword(_) => {
            items[rows[0]].action = RebaseAction::Reword;
            items[rows[0]].new_message = None;
        }
        QuickEdit::SquashIntoParent(_) | QuickEdit::Squash(_) => {
            if rows.windows(2).any(|w| w[1] != w[0] + 1) {
                return Err(refused(NOT_CONSECUTIVE));
            }
            let joined = rows.iter().map(|&r| items[r].message.trim()).collect::<Vec<_>>().join("\n\n");
            let first = rows[0];
            items[first].action = RebaseAction::Reword;
            items[first].new_message = Some(joined);
            for &r in &rows[1..] {
                items[r].action = RebaseAction::Fixup;
            }
        }
        QuickEdit::Drop(_) => {
            for &r in &rows {
                items[r].action = RebaseAction::Drop;
            }
            if p.upstream == Upstream::Root && rows.len() == items.len() {
                return Err(refused(DROPS_EVERYTHING));
            }
        }
    }
    if let Some(why) = problem(&items) {
        return Err(Error::Unsupported(why));
    }
    let last = rows[rows.len() - 1];
    let later = items.len() - 1 - last;
    let request = StartRequest { head: p.head, upstream: p.upstream, items, autostash: false };
    Ok(QuickPlan { edit: edit.clone(), request, branch: p.branch, edited: rows, later })
}

/// Walks HEAD's first-parent line until every target is found and returns the oldest one.
/// Refused for a merge on the way (chosen or not) and for targets the line never reaches.
fn oldest_on_branch(repo: &Repository, head: Oid, targets: &[Oid], many: bool) -> Result<Oid> {
    let mut left: HashSet<Oid> = targets.iter().copied().collect();
    let mut at = head;
    loop {
        let commit = repo.find_commit(at)?;
        let chosen = left.remove(&at);
        if commit.parent_count() > 1 {
            return Err(if chosen { refused(if many { SOME_MERGE_SELECTED } else { MERGE_SELECTED }) } else { merge_refused(at) });
        }
        if left.is_empty() {
            return Ok(at);
        }
        match commit.parent_ids().next() {
            Some(p) => at = p,
            None => return Err(refused(if many { SOME_NOT_ON_BRANCH } else { NOT_ON_BRANCH })),
        }
    }
}

fn refused(text: &str) -> Error {
    Error::Unsupported(text.into())
}

//! History edits chosen in the graph (reword, squash, drop), turned into a ready
//! [`StartRequest`] for the same `git rebase -i` run the panel uses.

use super::plan::{checked_head, plan};
use super::todo::problem;
use super::{
    BRANCH_TIP, DROPS_EVERYTHING, MERGE_SELECTED, NOT_CONSECUTIVE, NOT_ON_BRANCH, PARENT_MERGE, ROOT_SQUASH, RebaseAction, RebaseBase,
    SIDE_LINE, SOME_BRANCH_TIPS, SOME_MERGE_SELECTED, SOME_NOT_ON_BRANCH, SOME_SIDE_LINE, SPLIT_BY_BRANCH, StartRequest, TOO_FEW_TO_SQUASH,
    TodoStep, Upstream,
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
    /// The rows of `request.items` the edit changes, oldest first (every one is a commit row).
    pub edited: Vec<usize>,
    /// Commits (and merges) after the last edited row, which are rewritten too.
    pub later: usize,
}

/// The rebase that makes `edit` on the current branch. Merges after the commits are kept (the
/// list recreates them). Refused (`Error::Unsupported`, with a message for the user) while an
/// operation is in progress, on a detached HEAD, for commits that aren't on the branch's
/// first-parent line (`SIDE_LINE` for those that came in through a merge), for merges chosen,
/// for a squash of the first commit or into a merge, for a squash of commits that don't follow
/// one another or that another line starts between, and for dropping every commit of the
/// branch.
pub fn prepare_edit(workdir: &Path, edit: &QuickEdit) -> Result<QuickPlan> {
    let repo = Repository::open(workdir)?;
    let (head, _) = checked_head(&repo)?;
    let targets = edit.targets();
    let many = targets.len() > 1;
    if matches!(edit, QuickEdit::Squash(_)) && !many {
        return Err(refused(TOO_FEW_TO_SQUASH));
    }
    let (oldest, mut line) = oldest_on_branch(&repo, head, &targets, many)?;
    let mut chosen = targets.clone();
    let base = match edit {
        QuickEdit::SquashIntoParent(o) => {
            let commit = repo.find_commit(*o)?;
            let parent = commit.parent_ids().next().ok_or_else(|| refused(ROOT_SQUASH))?;
            if repo.find_commit(parent)?.parent_count() > 1 {
                return Err(refused(PARENT_MERGE));
            }
            chosen.push(parent);
            line.push(parent);
            parent
        }
        _ => oldest,
    };
    let p = plan(workdir, &RebaseBase::FromCommit(base))?;
    if let Some(h) = p.held.first() {
        return Err(Error::Unsupported(h.text()));
    }
    let mut steps = p.steps;
    let mut rows: Vec<usize> = chosen.iter().filter_map(|o| steps.iter().position(|s| s.oid() == Some(*o))).collect();
    rows.sort_unstable();
    rows.dedup();
    if rows.is_empty() {
        return Err(refused(if many { SOME_NOT_ON_BRANCH } else { NOT_ON_BRANCH }));
    }
    match edit {
        QuickEdit::Reword(_) => {
            if let Some(i) = steps[rows[0]].commit_mut() {
                i.action = RebaseAction::Reword;
                i.new_message = None;
            }
        }
        QuickEdit::SquashIntoParent(_) | QuickEdit::Squash(_) => {
            if rows.windows(2).any(|w| w[1] != w[0] + 1) {
                return Err(refused(if consecutive_on(&line, &chosen) { SPLIT_BY_BRANCH } else { NOT_CONSECUTIVE }));
            }
            let joined =
                rows.iter().filter_map(|&r| steps[r].commit()).map(|i| i.message.trim().to_owned()).collect::<Vec<_>>().join("\n\n");
            if let Some(i) = steps[rows[0]].commit_mut() {
                i.action = RebaseAction::Reword;
                i.new_message = Some(joined);
            }
            for &r in &rows[1..] {
                if let Some(i) = steps[r].commit_mut() {
                    i.action = RebaseAction::Fixup;
                }
            }
        }
        QuickEdit::Drop(_) => {
            // Another branch pointing at a dropped commit would lose it (rebase.updateRefs).
            if rows.iter().any(|&r| matches!(steps.get(r + 1), Some(TodoStep::UpdateRef(_)))) {
                return Err(refused(if many { SOME_BRANCH_TIPS } else { BRANCH_TIP }));
            }
            for &r in &rows {
                if let Some(i) = steps[r].commit_mut() {
                    i.action = RebaseAction::Drop;
                }
            }
            let commits = steps.iter().filter(|s| s.oid().is_some()).count();
            if p.upstream == Upstream::Root && rows.len() == commits {
                return Err(refused(DROPS_EVERYTHING));
            }
        }
    }
    let last = rows[rows.len() - 1];
    let later = steps[last + 1..].iter().filter(|s| s.oid().is_some()).count();
    if let Some(why) = problem(&steps) {
        return Err(Error::Unsupported(why));
    }
    let request = StartRequest { head: p.head, upstream: p.upstream, items: steps, autostash: false };
    Ok(QuickPlan { edit: edit.clone(), request, branch: p.branch, edited: rows, later })
}

/// Whether `oids` are consecutive commits of `line` (HEAD's first-parent line, newest first).
fn consecutive_on(line: &[Oid], oids: &[Oid]) -> bool {
    let mut at: Vec<usize> = oids.iter().filter_map(|o| line.iter().position(|l| l == o)).collect();
    at.sort_unstable();
    at.dedup();
    at.len() == oids.iter().collect::<HashSet<_>>().len() && at.windows(2).all(|w| w[1] == w[0] + 1)
}

/// Walks HEAD's first-parent line (through merges that weren't chosen) until every target is
/// found; returns the oldest target and the line from HEAD down to it, newest first. Refused
/// for a chosen merge, and for targets the line never reaches: `SIDE_LINE` when HEAD reaches
/// them through a merge, else `NOT_ON_BRANCH`.
fn oldest_on_branch(repo: &Repository, head: Oid, targets: &[Oid], many: bool) -> Result<(Oid, Vec<Oid>)> {
    let mut left: HashSet<Oid> = targets.iter().copied().collect();
    let mut line = Vec::new();
    let mut at = head;
    loop {
        let commit = repo.find_commit(at)?;
        line.push(at);
        if left.remove(&at) && commit.parent_count() > 1 {
            return Err(refused(if many { SOME_MERGE_SELECTED } else { MERGE_SELECTED }));
        }
        if left.is_empty() {
            return Ok((at, line));
        }
        match commit.parent_ids().next() {
            Some(p) => at = p,
            None => break,
        }
    }
    for t in &left {
        if repo.graph_descendant_of(head, *t)? {
            return Err(refused(if many { SOME_SIDE_LINE } else { SIDE_LINE }));
        }
    }
    Err(refused(if many { SOME_NOT_ON_BRANCH } else { NOT_ON_BRANCH }))
}

fn refused(text: &str) -> Error {
    Error::Unsupported(text.into())
}

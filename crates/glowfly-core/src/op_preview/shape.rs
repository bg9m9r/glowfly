//! The rows the previews share: a commit's line, the first-parent history afterwards, the
//! commits that arrive, and the commits nothing would reach any more.

use super::{AfterRow, CommitLine, Count, Gone, GoneWhy, Limits};
use crate::commit_text;
use crate::error::Result;
use git2::{Commit, Oid, Repository, Sort};
use std::collections::{HashMap, HashSet};

/// A commit as the preview shows it.
pub(super) fn line(c: &Commit<'_>) -> CommitLine {
    CommitLine { oid: c.id(), summary: commit_text::summary(c).into_owned(), author: commit_text::author(c).name, time: c.time().seconds() }
}

/// The line of the commit `oid`.
pub(super) fn line_of(repo: &Repository, oid: Oid) -> Result<CommitLine> {
    Ok(line(&repo.find_commit(oid)?))
}

/// The first-parent history from `tip`, newest first, at most `rows` rows after the `first`
/// ones (which count towards `rows`): a commit made in memory (`made`) is shown as the new
/// version of its original (a copy with `copy`), any other one as kept, arriving when it is in
/// `arrives`. Returns the rows and whether more history follows.
pub(super) fn after(
    repo: &Repository,
    first: Vec<AfterRow>,
    tip: Option<Oid>,
    made: &HashMap<Oid, Oid>,
    copy: bool,
    arrives: &HashSet<Oid>,
    rows: usize,
) -> Result<(Vec<AfterRow>, bool)> {
    let mut out = first;
    out.truncate(rows);
    let mut next = tip;
    while let Some(oid) = next {
        if out.len() == rows {
            return Ok((out, true));
        }
        let c = repo.find_commit(oid)?;
        out.push(match made.get(&oid) {
            Some(orig) => AfterRow::Rewritten { from: line_of(repo, *orig)?, copy },
            None => AfterRow::Kept { line: line(&c), arrives: arrives.contains(&oid) },
        });
        next = c.parent_ids().next();
    }
    Ok((out, false))
}

/// The commits reachable from `new` and not from `old`, up to `count` of them, and their count.
pub(super) fn arriving(repo: &Repository, new: Oid, old: Oid, count: usize) -> Result<(HashSet<Oid>, Count)> {
    let mut walk = repo.revwalk()?;
    walk.push(new)?;
    walk.hide(old)?;
    let mut set = HashSet::new();
    for id in walk {
        if set.len() == count {
            return Ok((set, Count { n: count, more: true }));
        }
        set.insert(id?);
    }
    let n = set.len();
    Ok((set, Count { n, more: false }))
}

/// The commits `old_head` reaches that neither `new_tip` nor any other ref (every ref under
/// `refs/` except the subject branch's own) nor another worktree's HEAD reaches, leaving out
/// the `moved` originals (they live on as their new versions), newest first: at most
/// `lim.rows` of them, with their count up to `lim.count`. Each has its reason from `reasons`,
/// else `LeftBehind`.
pub(super) fn gone(
    repo: &Repository,
    old_head: Oid,
    new_tip: Option<Oid>,
    subject: Option<&str>,
    moved: &[CommitLine],
    reasons: &HashMap<Oid, GoneWhy>,
    lim: &Limits,
) -> Result<(Vec<Gone>, Count)> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL)?;
    walk.push(old_head)?;
    if let Some(t) = new_tip {
        walk.hide(t)?;
    }
    let own = subject.map(|b| format!("refs/heads/{b}"));
    for r in repo.references()?.flatten() {
        if r.name_bytes().starts_with(b"refs/")
            && own.as_deref().is_none_or(|o| r.name_bytes() != o.as_bytes())
            && let Ok(c) = r.peel_to_commit()
        {
            walk.hide(c.id())?;
        }
    }
    for head in other_worktree_heads(repo) {
        walk.hide(head)?;
    }
    let moved: HashSet<Oid> = moved.iter().map(|m| m.oid).collect();
    let mut out = Vec::new();
    let mut n = 0;
    for id in walk {
        let id = id?;
        if moved.contains(&id) {
            continue;
        }
        if n == lim.count {
            return Ok((out, Count { n, more: true }));
        }
        n += 1;
        if out.len() < lim.rows {
            let why = reasons.get(&id).copied().unwrap_or(GoneWhy::LeftBehind);
            out.push(Gone { line: line_of(repo, id)?, why });
        }
    }
    Ok((out, Count { n, more: false }))
}

/// The HEAD commits of the repository's other worktrees (not this one's); unreadable ones are
/// left out.
fn other_worktree_heads(repo: &Repository) -> Vec<Oid> {
    let Ok(names) = repo.worktrees() else { return Vec::new() };
    let mut out = Vec::new();
    for name in names.iter().flatten() {
        let Ok(wt) = repo.find_worktree(name) else { continue };
        let Ok(other) = Repository::open_from_worktree(&wt) else { continue };
        if crate::paths::same_spelling(other.path(), repo.path()) {
            continue;
        }
        if let Some(head) = other.head().ok().and_then(|h| h.target()) {
            out.push(head);
        }
    }
    out
}

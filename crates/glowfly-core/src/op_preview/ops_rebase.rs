//! The rebase and drop previews: a replay of the commits onto the target (rebase) or of the
//! prepared drop's todo list (drop), stopping at the first step that conflicts or empties.

use super::super::replay::{Applied, Replay};
use super::super::shape::{arriving, gone, line, line_of};
use super::super::{AfterRow, CommitLine, GoneWhy, OCTOPUS, OpPreview, Outcome, REBASE_MERGES, STEPS, Stop, StopWhy, too_many};
use super::Cx;
use crate::error::{Error, Result};
use crate::rebase::{QuickEdit, RebaseAction, TodoStep, Upstream, prepare_edit};
use git2::{Oid, Repository, Sort};
use std::collections::{HashMap, HashSet};

/// `git rebase <onto>` (at `oid`): the commits `onto..HEAD` without merges and without the ones
/// whose patch the target already has, replayed onto it; a commit that becomes empty is left
/// out as already applied. Like git, a branch that contains the target is up to date only when
/// its history down to it is linear; a merge in between makes the rebase replay the rest.
pub(in super::super) fn rebase(cx: &Cx, onto: &str, oid: Oid) -> Result<OpPreview> {
    cx.check_ref(onto, oid)?;
    let (repo, head) = (cx.repo, cx.head);
    if cx.contains(head, oid)? && linear(repo, head, oid)? {
        return cx.nothing();
    }
    if repo.graph_descendant_of(oid, head)? {
        return cx.moves_to(Outcome::FastForward, oid);
    }
    let mut reasons: HashMap<Oid, GoneWhy> = HashMap::new();
    let mut picks: Vec<Oid> = Vec::new();
    for o in walk(repo, head, oid, Sort::TOPOLOGICAL | Sort::REVERSE)? {
        if repo.find_commit(o)?.parent_count() > 1 {
            reasons.insert(o, GoneWhy::MergeLeftOut);
        } else {
            picks.push(o);
        }
    }
    if !reasons.is_empty() && rebase_merges(repo) {
        return Err(Error::Unsupported(REBASE_MERGES.into()));
    }
    let mut skipped = false;
    let upstream: Vec<Oid> = walk(repo, oid, head, Sort::TOPOLOGICAL)?;
    let upstream: Vec<Oid> = upstream.into_iter().filter(|o| repo.find_commit(*o).is_ok_and(|c| c.parent_count() < 2)).collect();
    if upstream.len() > cx.lim.patch {
        skipped = true;
    } else {
        let mut ids = HashSet::new();
        for o in &upstream {
            if let Some(id) = patch_id(repo, *o)? {
                ids.insert(id);
            }
        }
        if !ids.is_empty() {
            let mut kept = Vec::with_capacity(picks.len());
            for o in picks {
                if patch_id(repo, o)?.is_some_and(|id| ids.contains(&id)) {
                    reasons.insert(o, GoneWhy::AlreadyApplied);
                } else {
                    kept.push(o);
                }
            }
            picks = kept;
        }
    }
    if picks.len() > cx.lim.replay {
        return Err(Error::Unsupported(too_many(picks.len())));
    }
    let mut r = Replay::new(repo, Some(oid))?;
    let mut moved: Vec<CommitLine> = Vec::new();
    let mut applied = 0;
    for (i, o) in picks.iter().enumerate() {
        let c = repo.find_commit(*o)?;
        match r.pick(&c, true, true)? {
            Applied::New(_) => {
                applied += 1;
                moved.push(line(&c));
            }
            Applied::Kept => applied += 1,
            Applied::Emptied => {
                reasons.insert(*o, GoneWhy::AlreadyApplied);
            }
            Applied::Conflicts(paths) => {
                let waiting = picks[i + 1..].iter().map(|w| line_of(repo, *w)).collect::<Result<_>>()?;
                let stop = Stop { why: StopWhy::Conflicts(paths), applied, waiting };
                let mut p = stopped(cx, &r, stop, line(&c))?;
                p.patch_check_skipped = skipped;
                return Ok(p);
            }
        }
    }
    let mut p = cx.base(Outcome::Done);
    p.patch_check_skipped = skipped;
    let (arrives, count) = arriving(repo, oid, head, cx.lim.count)?;
    p.arriving = Some(count);
    cx.rows(&mut p, Vec::new(), r.tip, &r.made, false, &arrives)?;
    finish(cx, &mut p, r.tip, moved, &reasons)?;
    Ok(p)
}

/// Dropping `oids` from the checked-out branch: the same todo list the real drop runs
/// (`prepare_edit`, so the same refusals), replayed from its upstream. As in `git rebase -i`,
/// a commit that becomes empty stops it.
pub(in super::super) fn drop(cx: &Cx, oids: &[Oid]) -> Result<OpPreview> {
    let plan = prepare_edit(cx.workdir, &QuickEdit::Drop(oids.to_vec()))?;
    let repo = cx.repo;
    let onto = match plan.request.upstream {
        Upstream::Commit(p) => Some(p),
        Upstream::Root => None,
    };
    let steps = &plan.request.items;
    let commits = steps.iter().filter(|s| s.oid().is_some()).count();
    if commits > cx.lim.replay {
        return Err(Error::Unsupported(too_many(commits)));
    }
    let mut r = Replay::new(repo, onto)?;
    let mut reasons: HashMap<Oid, GoneWhy> = HashMap::new();
    let mut moved: Vec<CommitLine> = Vec::new();
    let mut applied = 0;
    for (i, step) in steps.iter().enumerate() {
        let (c, result) = match step {
            TodoStep::Commit(item) if item.action == RebaseAction::Drop => {
                reasons.insert(item.oid, GoneWhy::Dropped);
                continue;
            }
            TodoStep::Commit(item) => {
                let c = repo.find_commit(item.oid)?;
                let result = r.pick(&c, true, true)?;
                (c, result)
            }
            TodoStep::Merge(m) if m.item.action == RebaseAction::Drop => {
                reasons.insert(m.item.oid, GoneWhy::Dropped);
                continue;
            }
            TodoStep::Merge(m) => {
                let [parent] = m.parents.as_slice() else { return Err(Error::Unsupported(OCTOPUS.into())) };
                let other = r.parent(parent)?;
                let c = repo.find_commit(m.item.oid)?;
                let result = r.merge_step(&c, other)?;
                (c, result)
            }
            TodoStep::Label { name, .. } => {
                r.label(name);
                continue;
            }
            TodoStep::Reset(to) => {
                r.reset(to)?;
                continue;
            }
            TodoStep::UpdateRef(_) => continue,
            TodoStep::Exec(_) | TodoStep::Break => return Err(Error::Unsupported(STEPS.into())),
        };
        let why = match result {
            Applied::New(_) => {
                applied += 1;
                moved.push(line(&c));
                continue;
            }
            Applied::Kept => {
                applied += 1;
                continue;
            }
            Applied::Emptied => StopWhy::Empty,
            Applied::Conflicts(paths) => StopWhy::Conflicts(paths),
        };
        let waiting = steps[i + 1..]
            .iter()
            .filter_map(TodoStep::item)
            .filter(|it| it.action != RebaseAction::Drop)
            .map(|it| line_of(repo, it.oid))
            .collect::<Result<_>>()?;
        return stopped(cx, &r, Stop { why, applied, waiting }, line(&c));
    }
    let mut p = cx.base(Outcome::Done);
    cx.rows(&mut p, Vec::new(), r.tip, &r.made, false, &HashSet::new())?;
    finish(cx, &mut p, r.tip, moved, &reasons)?;
    Ok(p)
}

/// A replay that stops at `at`: the branch stays where it is, so nothing is gone yet; the
/// history shows the stop on top of what was applied.
fn stopped(cx: &Cx, r: &Replay, stop: Stop, at: CommitLine) -> Result<OpPreview> {
    let mut p = cx.base(Outcome::Stopped(stop));
    cx.rows(&mut p, vec![AfterRow::Stopped { at: Some(at) }], r.tip, &r.made, false, &HashSet::new())?;
    Ok(p)
}

/// The rewritten originals and what nothing reaches once the branch is at `tip`.
fn finish(cx: &Cx, p: &mut OpPreview, tip: Option<Oid>, moved: Vec<CommitLine>, reasons: &HashMap<Oid, GoneWhy>) -> Result<()> {
    let (g, count) = gone(cx.repo, cx.head, tip, cx.subject.as_deref(), &moved, reasons, cx.lim)?;
    p.moved = moved;
    p.gone = g;
    p.gone_count = count;
    Ok(())
}

/// Whether the first-parent walk from `from` down to `to` (or to a root) meets no merge: the
/// check git makes before calling a branch that contains its target up to date.
fn linear(repo: &Repository, from: Oid, to: Oid) -> Result<bool> {
    let mut at = repo.find_commit(from)?;
    while at.id() != to {
        match at.parent_count() {
            0 => return Ok(true),
            1 => at = at.parent(0)?,
            _ => return Ok(false),
        }
    }
    Ok(true)
}

/// The commits `from` reaches and `hide` doesn't, in `sort` order.
fn walk(repo: &Repository, from: Oid, hide: Oid, sort: Sort) -> Result<Vec<Oid>> {
    let mut w = repo.revwalk()?;
    w.set_sorting(sort)?;
    w.push(from)?;
    w.hide(hide)?;
    Ok(w.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// The patch id of a non-merge commit's change; None for one that changes nothing.
fn patch_id(repo: &Repository, oid: Oid) -> Result<Option<Oid>> {
    let c = repo.find_commit(oid)?;
    let tree = c.tree()?;
    let parent = match c.parent_ids().next() {
        Some(p) => Some(repo.find_commit(p)?.tree()?),
        None => None,
    };
    if parent.as_ref().is_some_and(|p| p.id() == tree.id()) {
        return Ok(None);
    }
    let diff = repo.diff_tree_to_tree(parent.as_ref(), Some(&tree), None)?;
    Ok(Some(diff.patchid(None)?))
}

/// Whether `rebase.rebaseMerges` makes a plain rebase recreate merges.
fn rebase_merges(repo: &Repository) -> bool {
    let Ok(cfg) = repo.config().and_then(|mut c| c.snapshot()) else { return false };
    match cfg.get_string("rebase.rebasemerges") {
        Ok(v) => {
            let v = v.trim();
            v.eq_ignore_ascii_case("rebase-cousins")
                || v.eq_ignore_ascii_case("no-rebase-cousins")
                || git2::Config::parse_bool(v).unwrap_or(false)
        }
        Err(_) => false,
    }
}

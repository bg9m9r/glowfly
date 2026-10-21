//! The preview dialog's data and its pure rules: the subject and the refs it names, whether the
//! preview is out of date, its title and headline, the tags of its rows, why Apply is off, and
//! the message Apply sends, which is exactly what the operation's direct menu item sends.

use crate::effect::OpPreviewJob;
use crate::msg::{Msg, RebaseMsg, ResetMode, SelectionMsg};
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::op_preview::{AfterRow, CommitLine, GoneWhy, OpPreview, Outcome, PreviewOp, StopWhy};
use glowfly_core::rebase::QuickEdit;
use glowfly_core::repo::{CommitInfo, Snapshot};

/// The open preview: what was asked for and, once worked out, its result.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewDialog {
    pub job: OpPreviewJob,
    /// None while it is worked out.
    pub result: Option<Result<OpPreview, String>>,
}

/// The branch the operation moves: the checked-out one, or "HEAD".
pub fn subject(snap: &Snapshot) -> String {
    snap.head_branch.clone().unwrap_or_else(|| "HEAD".to_owned())
}

/// A full commit id ("1a2b…", 40 or 64 hex digits).
fn is_sha(name: &str) -> bool {
    matches!(name.len(), 40 | 64) && name.bytes().all(|b| b.is_ascii_hexdigit())
}

fn short(oid: Oid) -> String {
    oid.to_string()[..7].to_owned()
}

/// A name as the texts show it: a full sha cut to seven digits.
fn shown(name: &str) -> String {
    if is_sha(name) { name[..7].to_owned() } else { name.to_owned() }
}

/// The commit `name` names in the snapshot: a local branch, "remote/branch", or a full sha.
pub fn resolve(snap: &Snapshot, name: &str) -> Option<Oid> {
    if let Some(o) = snap.local_oid(name) {
        return Some(o);
    }
    let remote =
        snap.remotes.iter().find_map(|(r, _)| name.strip_prefix(r.as_str())?.strip_prefix('/').and_then(|b| snap.remote_oid(r, b)));
    remote.or_else(|| is_sha(name).then(|| Oid::from_str(name).ok()).flatten())
}

/// Whether HEAD or the ref the operation names moved since the preview was asked for.
pub fn stale(snap: &Snapshot, job: &OpPreviewJob) -> bool {
    if snap.head_oid != Some(job.request.head) {
        return true;
    }
    match &job.request.op {
        PreviewOp::Merge { theirs: name, oid } | PreviewOp::Rebase { onto: name, oid } => resolve(snap, name) != Some(*oid),
        PreviewOp::Reset { .. } | PreviewOp::CherryPick(_) | PreviewOp::Drop(_) => false,
    }
}

/// The dialog's title: "Preview: merge feature into main".
pub fn title(t: Lang, snap: &Snapshot, op: &PreviewOp) -> String {
    let branch = subject(snap);
    match op {
        PreviewOp::Merge { theirs, .. } => t.trf("op_preview.title.merge", &[("theirs", &shown(theirs)), ("branch", &branch)]),
        PreviewOp::Rebase { onto, .. } => t.trf("op_preview.title.rebase", &[("branch", &branch), ("onto", &shown(onto))]),
        PreviewOp::Reset { to } => t.trf("op_preview.title.reset", &[("branch", &branch), ("commit", &short(*to))]),
        PreviewOp::CherryPick(oids) => t.trn("op_preview.title.cherry_pick", oids.len(), &[("branch", &branch)]),
        PreviewOp::Drop(oids) => t.trn("op_preview.title.drop", oids.len(), &[("branch", &branch)]),
    }
}

/// `"Fix parser" (1a2b3c4)`, or the short sha alone for a commit without a summary.
pub fn commit_name(c: &CommitLine) -> String {
    let summary = c.summary.trim();
    if summary.is_empty() { short(c.oid) } else { format!("\"{summary}\" ({})", short(c.oid)) }
}

/// What the operation does, in a line or a few.
pub fn headline(t: Lang, p: &OpPreview) -> Vec<String> {
    let branch = p.subject.clone().unwrap_or_else(|| "HEAD".to_owned());
    let arrived = p.arriving.map_or(0, |c| c.n);
    let mut lines = Vec::new();
    match (&p.op, &p.outcome) {
        (PreviewOp::Merge { theirs, .. }, Outcome::Nothing) => {
            lines.push(t.trf("op_preview.nothing.merge", &[("branch", &branch), ("theirs", &shown(theirs))]))
        }
        (PreviewOp::Rebase { onto, .. }, Outcome::Nothing) => {
            lines.push(t.trf("op_preview.nothing.rebase", &[("branch", &branch), ("onto", &shown(onto))]))
        }
        (PreviewOp::Reset { .. }, Outcome::Nothing) => lines.push(t.trf("op_preview.nothing.reset", &[("branch", &branch)])),
        (PreviewOp::CherryPick(_) | PreviewOp::Drop(_), Outcome::Nothing) => lines.push(t.tr("op_preview.nothing_to_do").to_owned()),
        (PreviewOp::Merge { theirs: name, .. } | PreviewOp::Rebase { onto: name, .. }, Outcome::FastForward) => {
            lines.push(t.trn("op_preview.fast_forward", arrived, &[("branch", &branch), ("target", &shown(name))]))
        }
        (PreviewOp::Merge { .. }, Outcome::Done) => lines.push(t.tr("op_preview.done.merge").to_owned()),
        (PreviewOp::Merge { .. }, Outcome::Stopped(stop)) => {
            let n = match &stop.why {
                StopWhy::Conflicts(paths) => paths.len(),
                StopWhy::Empty => 0,
            };
            lines.push(t.trn("op_preview.stops.merge_conflicts", n, &[]));
        }
        (PreviewOp::Reset { to }, _) => lines.push(t.trf("op_preview.done.reset", &[("branch", &branch), ("commit", &short(*to))])),
        (PreviewOp::Rebase { onto, .. }, _) => {
            let n = p.moved.len() + stopped_applied(p) + stop_waiting(p) + usize::from(matches!(p.outcome, Outcome::Stopped(_)));
            lines.push(t.trn("op_preview.done.rebase", n, &[("onto", &shown(onto))]));
        }
        (PreviewOp::CherryPick(oids), _) => lines.push(t.trn("op_preview.done.cherry_pick", oids.len(), &[("branch", &branch)])),
        (PreviewOp::Drop(oids), _) => {
            lines.push(t.trn("op_preview.done.drop", oids.len(), &[]));
            if !p.moved.is_empty() {
                lines.push(t.trn("op_preview.rewrites", p.moved.len(), &[]));
            }
        }
    }
    if let Outcome::Stopped(stop) = &p.outcome {
        if !matches!(p.op, PreviewOp::Merge { .. })
            && let Some(at) = stop_commit(p)
        {
            let name = commit_name(at);
            let mut line = match &stop.why {
                StopWhy::Conflicts(paths) => t.trn("op_preview.stops.conflicts", paths.len(), &[("commit", &name)]),
                StopWhy::Empty => t.trf("op_preview.stops.empty", &[("commit", &name)]),
            };
            if stop.applied > 0 {
                line.push(' ');
                line.push_str(&t.trn("op_preview.applied", stop.applied, &[]));
            }
            if !stop.waiting.is_empty() {
                line.push(' ');
                line.push_str(&t.trn("op_preview.waiting", stop.waiting.len(), &[]));
            }
            lines.push(line);
        }
        // Each cherry-pick commits at once; the others move the branch only at the end.
        if !matches!(p.op, PreviewOp::CherryPick(_)) {
            lines.push(t.trf("op_preview.branch_waits", &[("branch", &branch)]));
        }
    }
    lines
}

fn stopped_applied(p: &OpPreview) -> usize {
    match &p.outcome {
        Outcome::Stopped(s) => s.applied,
        _ => 0,
    }
}

fn stop_waiting(p: &OpPreview) -> usize {
    match &p.outcome {
        Outcome::Stopped(s) => s.waiting.len(),
        _ => 0,
    }
}

/// The commit a replay stops at (the history's first row).
fn stop_commit(p: &OpPreview) -> Option<&CommitLine> {
    p.after.iter().find_map(|r| match r {
        AfterRow::Stopped { at } => at.as_ref(),
        _ => None,
    })
}

/// The conflicted paths a stop lists.
pub fn conflicts(p: &OpPreview) -> Option<&[String]> {
    match &p.outcome {
        Outcome::Stopped(s) => match &s.why {
            StopWhy::Conflicts(paths) if !paths.is_empty() => Some(paths),
            _ => None,
        },
        _ => None,
    }
}

/// The stop row's tag: "conflicts" or "empty".
pub fn stop_tag(t: Lang, p: &OpPreview) -> &'static str {
    match &p.outcome {
        Outcome::Stopped(s) if s.why == StopWhy::Empty => t.tr("op_preview.tag.empty"),
        _ => t.tr("op_preview.tag.conflict"),
    }
}

/// A gone commit's reason.
pub fn gone_tag(t: Lang, why: GoneWhy) -> &'static str {
    match why {
        GoneWhy::Dropped => t.tr("op_preview.gone.dropped"),
        GoneWhy::AlreadyApplied => t.tr("op_preview.gone.already_applied"),
        GoneWhy::MergeLeftOut => t.tr("op_preview.gone.merge"),
        GoneWhy::LeftBehind => t.tr("op_preview.gone.left_behind"),
    }
}

/// The weak notes under the rows: the rebase's skipped patch check, and uncommitted changes.
pub fn notes(t: Lang, snap: &Snapshot, p: &OpPreview) -> Vec<String> {
    let mut out = Vec::new();
    if p.patch_check_skipped
        && let PreviewOp::Rebase { onto, .. } = &p.op
    {
        out.push(t.trf("op_preview.note.patch_skipped", &[("onto", &shown(onto))]));
    }
    if !snap.staged.is_empty() || !snap.unstaged.is_empty() {
        out.push(t.tr("op_preview.note.dirty").to_owned());
    }
    out
}

/// Why these commits can't be cherry-picked through the preview: what blocks the direct item,
/// else a merge among them.
pub fn pick_blocker(t: Lang, snap: &Snapshot, oids: &[Oid]) -> Option<String> {
    if let Some(b) = crate::state::selection::cherry_pick_blocker(snap, oids) {
        return Some(b.text(t).to_owned());
    }
    let merge = |o: &Oid| snap.row_of.get(o).is_some_and(|&r| snap.commits[r].parents.len() > 1);
    oids.iter().any(merge).then(|| t.tr("op_preview.blocked.pick_merge").to_owned())
}

/// Why Apply is off: a job runs, the result is still worked out, the branch or the named ref
/// moved since, or there is nothing to do.
pub fn apply_blocker(t: Lang, job_running: bool, snap: &Snapshot, d: &PreviewDialog) -> Option<String> {
    if job_running {
        return Some(crate::state::blocked::busy(t).to_owned());
    }
    if d.result.is_none() {
        return Some(t.tr("op_preview.loading").to_owned());
    }
    if stale(snap, &d.job) {
        return Some(t.tr("op_preview.stale").to_owned());
    }
    matches!(&d.result, Some(Ok(p)) if p.outcome == Outcome::Nothing).then(|| t.tr("op_preview.nothing_to_do").to_owned())
}

/// What Apply sends: the operation's direct menu item's own message (`mode` picks a reset's;
/// None for a reset sends nothing).
pub fn apply_msg(t: Lang, snap: &Snapshot, op: &PreviewOp, mode: Option<ResetMode>) -> Option<Msg> {
    Some(match op {
        PreviewOp::Merge { theirs, .. } => Msg::git("Merge", &["merge", theirs]),
        PreviewOp::Rebase { onto, .. } => Msg::git("Rebase", &["rebase", onto]),
        PreviewOp::Reset { to } => {
            let sha = to.to_string();
            match mode? {
                ResetMode::Soft => Msg::git("Reset (soft)", &["reset", "--soft", &sha]),
                ResetMode::Mixed => Msg::git("Reset (mixed)", &["reset", "--mixed", &sha]),
                ResetMode::Hard => {
                    let loaded = snap.row_of.get(to).map(|&r| snap.commits[r].clone());
                    let c = loaded.unwrap_or_else(|| CommitInfo {
                        oid: *to,
                        summary: String::new(),
                        body: String::new(),
                        author: String::new(),
                        email: String::new(),
                        time: 0,
                        parents: Vec::new(),
                    });
                    crate::state::commit_menu::hard_reset_msg(t, snap, &c)
                }
            }
        }
        PreviewOp::CherryPick(oids) => Msg::Selection(SelectionMsg::CherryPick(oids.clone())),
        PreviewOp::Drop(oids) => Msg::Rebase(RebaseMsg::Edit(QuickEdit::Drop(oids.clone()))),
    })
}

#[cfg(test)]
#[path = "op_preview_tests.rs"]
mod tests;

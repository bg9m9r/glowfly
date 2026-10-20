//! The journal's Restore (pure): what restoring a state does to its subject, every reason it
//! can't, and the confirmation that runs it. Every path is undoable: the checked-out branch is
//! reset hard (its own stash keeps any discards), another branch is moved with `update-ref`
//! (`UndoKind::SetBranch`), a deleted branch is made again, and HEAD's own states are checked
//! out. On a dirty tree the reset and the checkouts stash first, approved in the same dialog.

use super::journal::short;
use crate::msg::{AutoStashMsg, JobMsg, Msg};
use crate::state::auto_stash::StashRetry;
use git2::{Oid, RepositoryState};
use trellis_core::i18n::Lang;
use trellis_core::journal::{Event, Op, Preview, Row, Subject, restore};
use trellis_core::repo::Snapshot;
use trellis_core::stash_ops::AutoStashFor;

/// What Restore does for a row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestorePlan {
    /// `branch` is checked out here: reset it (with the index and tree) from `from` to `to`.
    ResetHead { branch: String, from: Oid, to: Oid },
    /// `name` isn't checked out anywhere: move it from `from` to `to`.
    MoveBranch { name: String, from: Oid, to: Oid },
    /// `name` no longer exists: make it again at `at`.
    CreateBranch { name: String, at: Oid },
    /// A HEAD state that was a checkout of `name`, still at `to`: check it out. `from` is HEAD's
    /// commit now.
    CheckoutBranch { name: String, to: Oid, from: Option<Oid> },
    /// Any other HEAD state: detach HEAD at `to`.
    CheckoutDetached { to: Oid, from: Option<Oid> },
}

impl RestorePlan {
    pub fn command(&self) -> Vec<String> {
        match self {
            Self::ResetHead { to, .. } => restore::reset_to(*to),
            Self::MoveBranch { name, from, to } => restore::move_branch(name, *to, *from),
            Self::CreateBranch { name, at } => restore::create_branch(name, *at),
            Self::CheckoutBranch { name, .. } => restore::checkout_branch(name),
            Self::CheckoutDetached { to, .. } => restore::checkout_detached(*to),
        }
    }

    pub fn to(&self) -> Oid {
        match self {
            Self::ResetHead { to, .. }
            | Self::MoveBranch { to, .. }
            | Self::CheckoutBranch { to, .. }
            | Self::CheckoutDetached { to, .. } => *to,
            Self::CreateBranch { at, .. } => *at,
        }
    }

    /// The branch restored, or `None` for HEAD's own states.
    fn branch(&self) -> Option<&str> {
        match self {
            Self::ResetHead { branch: b, .. } | Self::MoveBranch { name: b, .. } | Self::CreateBranch { name: b, .. } => Some(b),
            Self::CheckoutBranch { .. } | Self::CheckoutDetached { .. } => None,
        }
    }

    /// Whether it changes the index and working tree.
    fn touches_tree(&self) -> bool {
        matches!(self, Self::ResetHead { .. } | Self::CheckoutBranch { .. } | Self::CheckoutDetached { .. })
    }

    /// The revision that must still be where it was: the branch's tip, or HEAD's commit.
    fn expect(&self) -> Option<(String, Oid)> {
        match self {
            Self::ResetHead { branch, from, .. } => Some((format!("refs/heads/{branch}"), *from)),
            Self::MoveBranch { name, from, .. } => Some((format!("refs/heads/{name}"), *from)),
            Self::CreateBranch { .. } => None,
            Self::CheckoutBranch { from, .. } | Self::CheckoutDetached { from, .. } => from.map(|o| ("HEAD".to_owned(), o)),
        }
    }
}

/// Whether a merge, rebase or other operation is in progress, or files are conflicted.
fn busy(snap: &Snapshot) -> bool {
    snap.state != RepositoryState::Clean || super::rebase::has_conflicts(snap)
}

/// What Restore would do for `row`, or why it can't now.
pub fn plan(snap: &Snapshot, row: &Row, job_running: bool, t: Lang) -> Result<RestorePlan, String> {
    if job_running {
        return Err(t.tr("commit.blocked.job_running").into());
    }
    if busy(snap) {
        return Err(t.tr("journal.blocked.in_progress").into());
    }
    if row.summary.is_none() {
        return Err(t.tr("journal.blocked.gone").into());
    }
    let to = row.oid;
    let current = |name: &str| t.trf("journal.blocked.current", &[("branch", &name)]);
    match &row.subject {
        Subject::Branch(name) => {
            let Some(tip) = snap.locals.iter().find(|b| b.name == *name).and_then(|b| b.oid) else {
                return Ok(RestorePlan::CreateBranch { name: name.clone(), at: to });
            };
            if tip == to {
                return Err(current(name));
            }
            if let Some(wt) = super::worktree_branches::elsewhere(snap, name) {
                let folder = trellis_core::native_path::shown(&wt.path);
                return Err(t.trf("journal.blocked.elsewhere", &[("branch", name), ("folder", &folder)]));
            }
            Ok(if snap.head_branch.as_deref() == Some(name) {
                RestorePlan::ResetHead { branch: name.clone(), from: tip, to }
            } else {
                RestorePlan::MoveBranch { name: name.clone(), from: tip, to }
            })
        }
        Subject::Head => {
            let from = snap.head_oid;
            if from == Some(to) {
                return Err(current("HEAD"));
            }
            let branch = match &row.event {
                Event::Checkout { to: name, .. } => snap
                    .locals
                    .iter()
                    .find(|b| b.name == *name && b.oid == Some(to))
                    .filter(|b| super::worktree_branches::elsewhere(snap, &b.name).is_none()),
                _ => None,
            };
            Ok(match branch {
                Some(b) => RestorePlan::CheckoutBranch { name: b.name.clone(), to, from },
                None => RestorePlan::CheckoutDetached { to, from },
            })
        }
    }
}

/// The sentences about the commits that leave and come back, once the preview has them.
fn counts(t: Lang, preview: Option<&Preview>) -> Vec<String> {
    let Some(p) = preview else { return Vec::new() };
    let mut out = Vec::new();
    if let Some(l) = p.leaving.filter(|c| c.n > 0) {
        out.push(if l.more { t.tr("journal.restore.leaving_more").into() } else { t.trn("journal.restore.leaving", l.n, &[]) });
    }
    if let Some(a) = p.arriving.filter(|c| c.n > 0) {
        out.push(if a.more { t.tr("journal.restore.arriving_more").into() } else { t.trn("journal.restore.arriving", a.n, &[]) });
    }
    out
}

/// The confirmation for `plan`: what moves, the counts from `preview` when it has loaded, what
/// happens to the working tree and that Undo puts it back. On a `dirty` tree a reset or
/// checkout stashes first (`head` is the checked-out branch, for the stash's message).
pub fn confirm(t: Lang, plan: &RestorePlan, preview: Option<&Preview>, dirty: bool, head: Option<&str>) -> Msg {
    let to = short(plan.to());
    let shown = plan.branch().unwrap_or("HEAD");
    let mut body = vec![match plan {
        RestorePlan::CreateBranch { .. } => t.trf("journal.restore.create_at", &[("branch", &shown), ("to", &to)]),
        RestorePlan::ResetHead { from, .. } | RestorePlan::MoveBranch { from, .. } => {
            t.trf("journal.restore.from_to", &[("branch", &shown), ("from", &short(*from)), ("to", &to)])
        }
        RestorePlan::CheckoutBranch { from, .. } | RestorePlan::CheckoutDetached { from, .. } => {
            let from = from.map(short).unwrap_or_default();
            t.trf("journal.restore.from_to", &[("branch", &shown), ("from", &from), ("to", &to)])
        }
    }];
    body.extend(counts(t, preview));
    body.push(if plan.touches_tree() { t.tr("journal.restore.tree_reset") } else { t.tr("journal.restore.tree_kept") }.into());
    let cmd = plan.command();
    let stash = (dirty && plan.touches_tree()).then(|| AutoStashFor::of_command(&cmd, head)).flatten();
    if stash.is_some() {
        body.push(t.tr("journal.restore.stash_note").into());
    }
    body.push(t.tr("journal.restore.undo_note").into());
    let (title, button) = match plan {
        RestorePlan::ResetHead { .. } | RestorePlan::MoveBranch { .. } => (
            t.trf("journal.restore.title", &[("branch", &shown)]),
            if stash.is_some() { t.tr("journal.restore.confirm_stash") } else { t.tr("journal.restore.confirm") },
        ),
        RestorePlan::CreateBranch { .. } => (t.trf("journal.restore.title_create", &[("branch", &shown)]), t.tr("journal.restore.create")),
        RestorePlan::CheckoutBranch { .. } | RestorePlan::CheckoutDetached { .. } => (
            t.tr("journal.restore.title_head").to_owned(),
            if stash.is_some() { t.tr("journal.restore.checkout_stash") } else { t.tr("journal.restore.checkout") },
        ),
    };
    let label = match plan.branch() {
        Some(b) => t.trf("journal.restore.job", &[("branch", &b), ("to", &to)]),
        None => t.trf("journal.restore.job_head", &[("to", &to)]),
    };
    let op = Op::Restore { branch: plan.branch().map(str::to_owned), to };
    let journaled = Msg::Job(JobMsg::Journaled { label, cmd, op, expect: plan.expect() });
    let then = match stash {
        Some(op) => Msg::AutoStash(AutoStashMsg::Retry(Box::new(StashRetry { op, retry: Box::new(journaled) }))),
        None => journaled,
    };
    Msg::confirm(&title, body.join(" "), button, then)
}

/// The menu's and the preview's Create branch here…: the New branch dialog at the row's
/// commit, named after a deleted branch.
pub fn create_branch(row: &Row, snap: &Snapshot) -> crate::msg::BranchMsg {
    let name = match &row.subject {
        Subject::Branch(b) if !snap.locals.iter().any(|l| l.name == *b) => b.clone(),
        _ => String::new(),
    };
    crate::msg::BranchMsg::NewBranch { start: Some(row.oid.to_string()), name }
}

#[cfg(test)]
#[path = "journal_restore_tests.rs"]
mod tests;

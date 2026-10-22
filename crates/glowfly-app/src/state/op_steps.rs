//! The WIP panel's steps while an operation is in progress (`ui/op_steps.rs`): 1 Resolve
//! conflicts, 2 Check what goes in, 3 Commit or Continue. Which operation it is, each step's
//! state (done, now, later), whether a step is open (its default, or the one the person chose
//! with `LayoutMsg::ToggleOpStep`), and the steps' words. Bisect and a bare `am`/rebase
//! leftover with nothing conflicted keep the plain card (`ui/details_banner.rs`): they have no
//! conflicts to resolve and nothing to commit.

use crate::state::op_actions::banner_title;
use git2::RepositoryState;
use glowfly_core::i18n::Lang;
use glowfly_core::rebase::StopKind;
use glowfly_core::repo::Snapshot;

/// The operation the steps are for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpKind {
    Merge,
    Rebase,
    CherryPick,
    Revert,
    /// A patch series (`git am`).
    Patches,
    /// Conflicts with no operation in progress (a stash that didn't apply cleanly).
    Conflicts,
}

/// One of the steps; `CommitForm` is the commit form inside step 3 of an operation that
/// continues rather than commits (shown for a rebase stopped to edit, else on request).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OpStep {
    Resolve,
    Check,
    Finish,
    CommitForm,
}

/// Where a step stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    Done,
    Now,
    Later,
}

/// The steps' operation for `snap`, or None for the plain panel.
pub fn op_kind(snap: &Snapshot) -> Option<OpKind> {
    use RepositoryState::*;
    Some(match snap.state {
        Merge => OpKind::Merge,
        Rebase | RebaseMerge | RebaseInteractive => OpKind::Rebase,
        CherryPick | CherryPickSequence => OpKind::CherryPick,
        Revert | RevertSequence => OpKind::Revert,
        ApplyMailbox => OpKind::Patches,
        Clean | Bisect | ApplyMailboxOrRebase if conflicted(snap) > 0 => OpKind::Conflicts,
        Clean | Bisect | ApplyMailboxOrRebase => return None,
    })
}

/// How many files are conflicted.
pub fn conflicted(snap: &Snapshot) -> usize {
    snap.unstaged.iter().filter(|f| f.status == '!').count()
}

/// Where `step` stands with `conflicted` files left: resolving comes first, then the rest.
pub fn step_state(step: OpStep, conflicted: usize) -> StepState {
    match (step, conflicted > 0) {
        (OpStep::Resolve, true) => StepState::Now,
        (OpStep::Resolve, false) | (OpStep::Check, false) => StepState::Done,
        (OpStep::Finish | OpStep::CommitForm, false) => StepState::Now,
        (OpStep::Check | OpStep::Finish | OpStep::CommitForm, true) => StepState::Later,
    }
}

/// Whether `step` is open when the person hasn't chosen: the step now is; Check stays open
/// while conflicts are left, so the files can be looked over meanwhile; done and later steps
/// fold to a line, but step 3 of an operation that continues stays open for its Skip. The commit form of an operation that continues opens by itself only where
/// committing is the point of the stop (a rebase paused to edit or at a break).
pub fn default_open(snap: &Snapshot, step: OpStep) -> bool {
    let n = conflicted(snap);
    match step {
        OpStep::Check => n > 0,
        OpStep::CommitForm => n == 0 && snap.rebase.as_ref().is_some_and(|p| matches!(p.stop.kind, StopKind::Edit | StopKind::Break)),
        // Skip and Edit remaining steps live there, so an operation that continues keeps it open.
        OpStep::Finish => n == 0 || !matches!(op_kind(snap), Some(OpKind::Merge | OpKind::Conflicts)),
        OpStep::Resolve => n > 0,
    }
}

/// The steps the person opened or folded, kept while the operation stays where it is: a new
/// operation, or the conflicts all resolved, starts from the defaults again.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StepFolds {
    at: Option<(RepositoryState, bool)>,
    chosen: Vec<(OpStep, bool)>,
}

impl StepFolds {
    /// Whether `step` is open for `snap`.
    pub fn is_open(&self, snap: &Snapshot, step: OpStep) -> bool {
        let chosen = (self.at == Some(where_at(snap))).then(|| self.chosen.iter().find(|(s, _)| *s == step)).flatten();
        chosen.map_or_else(|| default_open(snap, step), |(_, open)| *open)
    }

    /// Opens a folded `step`, or folds an open one.
    pub fn toggle(&mut self, snap: &Snapshot, step: OpStep) {
        let open = !self.is_open(snap, step);
        if self.at != Some(where_at(snap)) {
            self.at = Some(where_at(snap));
            self.chosen.clear();
        }
        self.chosen.retain(|(s, _)| *s != step);
        self.chosen.push((step, open));
    }
}

fn where_at(snap: &Snapshot) -> (RepositoryState, bool) {
    (snap.state, conflicted(snap) > 0)
}

/// The panel's heading: "Merge in progress", "Rebase in progress · step 2 of 4", or for
/// conflicts alone "Conflicts to resolve".
pub fn heading(t: Lang, snap: &Snapshot) -> String {
    match op_kind(snap) {
        Some(OpKind::Conflicts) | None => t.tr("op_strip.conflicts").to_owned(),
        Some(_) => banner_title(t, snap).unwrap_or_default(),
    }
}

/// Step 3's title: "Commit the merge", "Continue rebase · step 3 of 7", "Continue the
/// cherry-pick".
pub fn finish_title(t: Lang, snap: &Snapshot, kind: OpKind) -> String {
    let rebase = snap.rebase.as_ref().filter(|p| p.total > 0).map(|p| (p.step, p.total));
    let am = snap.am.as_ref().filter(|p| p.total > 0).map(|p| (p.step, p.total));
    match (kind, rebase, am) {
        (OpKind::Merge, _, _) => t.tr("details.steps.finish.merge").to_owned(),
        (OpKind::Rebase, Some((step, total)), _) => t.trf("details.steps.finish.rebase_step", &[("step", &step), ("total", &total)]),
        (OpKind::Rebase, None, _) => t.tr("details.steps.finish.rebase").to_owned(),
        (OpKind::CherryPick, _, _) => t.tr("details.steps.finish.cherry_pick").to_owned(),
        (OpKind::Revert, _, _) => t.tr("details.steps.finish.revert").to_owned(),
        (OpKind::Patches, _, Some((step, total))) => t.trf("details.steps.finish.am_step", &[("step", &step), ("total", &total)]),
        (OpKind::Patches, _, None) => t.tr("details.steps.finish.am").to_owned(),
        (OpKind::Conflicts, _, _) => t.tr("details.steps.finish.conflicts").to_owned(),
    }
}

/// Step 1's line: "2 left" while conflicts are, then "Conflicts resolved" (or "No conflicts"
/// when there never were any).
pub fn resolve_line(t: Lang, conflicted: usize, resolved: usize) -> String {
    match (conflicted, resolved) {
        (0, 0) => t.tr("details.steps.resolve.none").to_owned(),
        (0, _) => t.tr("details.steps.resolve.done").to_owned(),
        (n, _) => t.trn("details.steps.resolve.left", n, &[]),
    }
}

/// Step 2's folded line: how many files go in and how many stay out.
pub fn check_line(t: Lang, staged: usize, other: usize) -> String {
    t.trf("details.steps.check.summary", &[("staged", &staged), ("other", &other)])
}

/// Step 2's note: other changes stay out unless staged.
pub fn check_note(t: Lang, kind: OpKind) -> &'static str {
    match kind {
        OpKind::Merge => t.tr("details.steps.check.note.merge"),
        _ => t.tr("details.steps.check.note"),
    }
}

#[cfg(test)]
#[path = "op_steps_tests.rs"]
mod tests;

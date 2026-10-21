//! The palette's history entries: restore the open rebase panel's original order, reword,
//! squash into parent and drop the selected commit (when that edit can run), rebase the current
//! branch onto it (plainly or interactively), and Continue, Skip and Edit remaining steps while
//! a rebase is in progress.

use super::{Entry, PaletteCommand as C, entry};
use crate::keymap::BindingId;
use crate::msg::{Msg, RebaseMsg};
use crate::state::rebase::has_conflicts;
use crate::state::{AppState, Selection};
use git2::{Oid, RepositoryState};
use glowfly_core::rebase::{QuickEdit, RebaseBase, Resume};
use glowfly_core::repo::Snapshot;

#[derive(Clone, Debug, PartialEq)]
pub enum RebaseCommand {
    /// A history edit of the selected commit, as the graph's menu sends it.
    Edit(QuickEdit),
    /// The banner's Continue or Skip commit.
    Resume(Resume),
    /// Open the panel on the remaining steps of the interactive rebase in progress.
    EditRemaining,
    /// Rebase the current branch onto the commit: `git rebase <oid>`, or the panel.
    Onto { oid: Oid, interactive: bool },
    /// The open panel's list back to its original order and actions (a stale list reloads).
    RestoreOrder,
}

impl RebaseCommand {
    pub fn msg(&self) -> Msg {
        let m = match self {
            Self::Edit(e) => RebaseMsg::Edit(e.clone()),
            Self::Resume(how) => RebaseMsg::Resume(*how),
            Self::EditRemaining => RebaseMsg::OpenRemaining,
            Self::RestoreOrder => RebaseMsg::RebaseReset,
            Self::Onto { oid, interactive: true } => RebaseMsg::OpenRebase(RebaseBase::OntoCommit(*oid)),
            Self::Onto { oid, interactive: false } => return Msg::git("Rebase", &["rebase", &oid.to_string()]),
        };
        Msg::Rebase(m)
    }

    /// The shortcut that does the same.
    pub fn binding(&self) -> Option<BindingId> {
        Some(match self {
            Self::Edit(QuickEdit::Reword(_)) => BindingId::RewordCommit,
            Self::Edit(QuickEdit::SquashIntoParent(_) | QuickEdit::Squash(_)) => BindingId::SquashCommit,
            Self::Edit(QuickEdit::Drop(_)) => BindingId::DropCommit,
            Self::Resume(Resume::Continue) => BindingId::RebaseContinue,
            Self::Resume(Resume::Skip) => BindingId::RebaseSkip,
            Self::EditRemaining | Self::Onto { .. } | Self::RestoreOrder => return None,
        })
    }
}

pub(super) fn entries(st: &AppState, snap: &Snapshot, out: &mut Vec<Entry>) {
    let cmd = |c: RebaseCommand| C::Rebase(c);
    if let Some(p) = st.rebase.as_ref().filter(|p| p.editable()) {
        let label = if p.stale { "Reload rebase steps" } else { "Restore original order" };
        out.push(entry(cmd(RebaseCommand::RestoreOrder), label, "rebase reset undo revert original order steps todo"));
    }
    if let Selection::Commit(oid) = st.selection {
        let b = super::super::rebase_quick::single(snap, oid);
        let sha = &oid.to_string()[..7];
        if b.reword.is_none() {
            let c = cmd(RebaseCommand::Edit(QuickEdit::Reword(oid)));
            out.push(entry(c, format!("Reword commit {sha}…"), "rebase history message rename amend edit"));
        }
        if b.squash.is_none() {
            let c = cmd(RebaseCommand::Edit(QuickEdit::SquashIntoParent(oid)));
            out.push(entry(c, format!("Squash {sha} into parent"), "rebase history combine meld fixup merge into"));
        }
        if b.drop.is_none() {
            let c = cmd(RebaseCommand::Edit(QuickEdit::Drop(vec![oid])));
            out.push(entry(c, format!("Drop commit {sha}…"), "rebase history remove delete discard"));
        }
        if let (Some(current), None) = (&snap.head_branch, super::super::rebase_onto::onto_commit_blocker(st.settings.lang(), snap, oid)) {
            let (plain, interactive) = super::super::rebase_onto::onto_labels(current, oid);
            out.push(entry(cmd(RebaseCommand::Onto { oid, interactive: false }), plain, "rebase onto base move commit"));
            let c = cmd(RebaseCommand::Onto { oid, interactive: true });
            out.push(entry(c, interactive, "rebase interactive onto base todo commit"));
        }
    }
    use RepositoryState::*;
    if !matches!(snap.state, Rebase | RebaseInteractive | RebaseMerge | ApplyMailboxOrRebase) {
        return;
    }
    if !has_conflicts(snap) {
        out.push(entry(cmd(RebaseCommand::Resume(Resume::Continue)), "Continue rebase", "rebase resume go on next proceed"));
    }
    out.push(entry(cmd(RebaseCommand::Resume(Resume::Skip)), "Skip commit (rebase)", "rebase skip omit pass"));
    if snap.state == RebaseInteractive {
        out.push(entry(
            cmd(RebaseCommand::EditRemaining),
            "Edit remaining rebase steps…",
            "rebase todo steps exec break reorder edit-todo",
        ));
    }
}

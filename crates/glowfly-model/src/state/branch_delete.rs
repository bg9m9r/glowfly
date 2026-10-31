//! The branch Delete dialog (`Modal::DeleteBranch`, drawn by `ui/branch_delete_dialog.rs`): the
//! branch, the tip it opened for, how many commits only it has (worked out in the background by
//! `BranchEffect::CountUnmerged`, unless the checked-out branch already contains it) and its Git
//! Flow role. With nothing to lose it offers Delete; with commits only on it, a failed count or a
//! Git Flow base it offers Delete anyway instead. Cancel is the default button either way, so
//! Enter never deletes.

use crate::msg::{BranchMsg, Msg};
use crate::state::commit_menu::menu_trim;
use crate::state::gitflow_base;
use git2::Oid;
use glowfly_core::gitflow::FlowBase;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{BranchInfo, Snapshot};

/// What deleting the branch would leave on no branch.
#[derive(Clone, Debug, PartialEq)]
pub enum Lost {
    /// Still counting.
    Counting,
    /// This many commits are on no other branch (0: all of them are).
    Known(usize),
    /// The count failed, with git's error.
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeleteForm {
    pub name: String,
    /// The tip the dialog opened for: the delete is refused if the branch has moved since.
    pub tip: Oid,
    pub lost: Lost,
    /// The checked-out branch contains the tip, so `git branch -d` takes it.
    pub in_head: bool,
    pub role: Option<FlowBase>,
}

impl DeleteForm {
    /// The dialog for local branch `b`, which has a tip: known to lose nothing when the
    /// checked-out branch contains it, else waiting for the count.
    pub fn new(snap: &Snapshot, b: &BranchInfo, tip: Oid) -> Self {
        let in_head = snap.head_branch.is_some() && (b.merged || menu_trim::head_contains(snap, tip));
        let lost = if in_head { Lost::Known(0) } else { Lost::Counting };
        DeleteForm { name: b.name.clone(), tip, lost, in_head, role: snap.gitflow.base_role(&b.name) }
    }

    pub fn title(&self, t: Lang) -> &'static str {
        match self.role {
            Some(role) => gitflow_base::title(t, role),
            None => t.tr("branch.delete.title"),
        }
    }

    /// Whether commits only on the branch would be lost, or might be (the count failed).
    pub fn loses_work(&self) -> bool {
        !matches!(self.lost, Lost::Known(0) | Lost::Counting)
    }

    /// The danger button's label: Delete anyway when work may be lost or Git Flow needs the branch.
    pub fn button(&self, t: Lang) -> &'static str {
        if self.loses_work() {
            t.tr("branch.delete.anyway.button")
        } else if self.role.is_some() {
            t.tr("gitflow.delete_base.delete.button")
        } else {
            t.tr("common.delete.button")
        }
    }

    /// What may be lost, under the question: the count, or why it is unknown.
    pub fn lost_note(&self, t: Lang) -> Option<String> {
        match &self.lost {
            Lost::Counting | Lost::Known(0) => None,
            Lost::Known(n) => Some(t.trn("branch.delete.unmerged", *n, &[("branch", &self.name)])),
            Lost::Failed(e) => Some(t.trf("branch.delete.check_failed", &[("error", e)])),
        }
    }

    /// What the danger button sends; None while the count runs. A branch the checked-out one
    /// contains goes with `git branch -d`; any other with `-D`, which deletes it whether or not
    /// git calls it merged (its commits are on another branch, or the user chose Delete anyway).
    pub fn delete_msg(&self) -> Option<Msg> {
        (self.lost != Lost::Counting)
            .then(|| Msg::Branch(BranchMsg::Delete { name: self.name.clone(), tip: self.tip, force: !self.in_head }))
    }
}

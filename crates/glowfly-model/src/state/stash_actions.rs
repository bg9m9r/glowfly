//! The one builder of a listed stash's actions: the STASHES row menu, the palette's stash
//! sub-lists, and Unstash in the toolbar and the palette (the latest stash) all take their
//! messages, order and blocked reasons from here. Pure.

use super::stash_confirm::{StashAction, StashRun, apply_blocked, guarded};
use crate::msg::{Msg, StashMsg};
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{Snapshot, StashInfo};

/// What a stash's menu offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StashOption {
    Apply,
    Pop,
    Rename,
    Drop,
}

impl From<StashAction> for StashOption {
    fn from(action: StashAction) -> Self {
        match action {
            StashAction::Apply => Self::Apply,
            StashAction::Pop => Self::Pop,
            StashAction::Drop => Self::Drop,
        }
    }
}

/// Menu order.
const ALL: [StashOption; 4] = [StashOption::Apply, StashOption::Pop, StashOption::Rename, StashOption::Drop];

/// One action on a stash, with what choosing it sends.
pub struct StashItem {
    pub option: StashOption,
    /// Why it can't run now (greyed with this reason); None when it can.
    pub blocked: Option<&'static str>,
    /// The destructive item: danger style, in the menu's last group.
    pub danger: bool,
    /// What choosing it sends: the job, the question before it, the dialog, or (while
    /// blocked) the refusal line for entry points that can't grey an item.
    pub msg: Msg,
}

/// The actions on stash `s`, in menu order.
pub fn items(snap: &Snapshot, t: Lang, s: &StashInfo) -> [StashItem; 4] {
    ALL.map(|option| item(snap, t, option, s))
}

/// `option` on the stash with commit `oid` as `snap` lists it; None when it is gone.
pub fn for_commit(snap: &Snapshot, t: Lang, option: StashOption, oid: Oid) -> Option<Msg> {
    let s = snap.stashes.iter().find(|s| s.oid == oid)?;
    Some(item(snap, t, option, s).msg)
}

/// Pop of the newest stash as the snapshot shows it (`stash@{0}` with its commit); None
/// without stashes.
pub fn pop_latest(snap: &Snapshot, t: Lang) -> Option<Msg> {
    Some(item(snap, t, StashOption::Pop, snap.stashes.first()?).msg)
}

fn item(snap: &Snapshot, t: Lang, option: StashOption, s: &StashInfo) -> StashItem {
    let run = |action| guarded(snap, t, action, s.oid, &s.message, StashRun::Listed { index: s.index });
    let (blocked, danger, msg) = match option {
        StashOption::Apply => (apply_blocked(snap, t), false, run(StashAction::Apply)),
        StashOption::Pop => (apply_blocked(snap, t), false, run(StashAction::Pop)),
        StashOption::Rename => (None, false, Msg::Stash(StashMsg::RenameDialog { oid: s.oid })),
        StashOption::Drop => (None, true, run(StashAction::Drop)),
    };
    StashItem { option, blocked, danger, msg }
}

#[cfg(test)]
#[path = "stash_actions_tests.rs"]
mod tests;

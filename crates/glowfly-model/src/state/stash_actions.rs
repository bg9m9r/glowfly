//! The one builder of a listed stash's actions: the STASHES row menu, the palette's stash
//! sub-lists, and Unstash in the toolbar and the palette (the latest stash) all take their
//! messages, order and blocked reasons from here. Pure. Apply and Unstash bring back the staged
//! state too (update/stash.rs).

use super::clock::Clock;
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

/// The actions on stash `s`, in menu order. Delete's question names the day the stash was made
/// (`clock`).
pub fn items(snap: &Snapshot, clock: Clock<'_>, s: &StashInfo) -> [StashItem; 4] {
    ALL.map(|option| item(snap, clock.lang(), Some(clock), option, s))
}

/// `option` on the stash with commit `oid` as `snap` lists it; None when it is gone. Delete's
/// question names the day the stash was made (`clock`).
pub fn for_commit(snap: &Snapshot, clock: Clock<'_>, option: StashOption, oid: Oid) -> Option<Msg> {
    let s = snap.stashes.iter().find(|s| s.oid == oid)?;
    Some(item(snap, clock.lang(), Some(clock), option, s).msg)
}

/// Pop of the newest stash as the snapshot shows it (`stash@{0}` with its commit); None
/// without stashes. Pop never names a date.
pub fn pop_latest(snap: &Snapshot, t: Lang) -> Option<Msg> {
    Some(item(snap, t, None, StashOption::Pop, snap.stashes.first()?).msg)
}

/// `clock` is None where no date is named (Pop; Delete then leaves its question's date out).
fn item(snap: &Snapshot, t: Lang, clock: Option<Clock<'_>>, option: StashOption, s: &StashInfo) -> StashItem {
    let run = |action, when: Option<&str>| guarded(snap, t, action, s.oid, &s.message, when, StashRun::Listed { index: s.index });
    let (blocked, danger, msg) = match option {
        StashOption::Apply => (apply_blocked(snap, t), false, run(StashAction::Apply, None)),
        StashOption::Pop => (apply_blocked(snap, t), false, run(StashAction::Pop, None)),
        StashOption::Rename => (None, false, Msg::Stash(StashMsg::RenameDialog { oid: s.oid })),
        StashOption::Drop => {
            let when = clock.zip(s.time).map(|(clock, secs)| clock.day(secs));
            (None, true, run(StashAction::Drop, when.as_deref()))
        }
    };
    StashItem { option, blocked, danger, msg }
}

#[cfg(test)]
#[path = "stash_actions_tests.rs"]
mod tests;

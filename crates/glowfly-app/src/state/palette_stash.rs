//! The palette's stash entries: Stash changes, Stash with message…, the Stash dialog's Stash
//! staged changes… and Stash selected files… (while the tree is dirty), then Pop stash and the
//! apply, pop, delete and rename sub-lists (while there are stashes).

use super::{Entry, PaletteCommand as C, entry, with_detail};
use crate::state::palette_kinds::StashAction;
use crate::state::stashes::wip_files;
use glowfly_core::repo::Snapshot;
use glowfly_core::stash_ops::StashScope;

pub(super) fn stash_entries(snap: &Snapshot, out: &mut Vec<Entry>) {
    if snap.is_dirty() {
        out.push(entry(C::Stash, "Stash changes", "save shelve park untracked wip stash push"));
        out.push(entry(C::StashMessage, "Stash with message…", "save shelve park untracked wip stash push name describe message"));
        let staged = match snap.staged.len() {
            0 => "nothing staged".to_owned(),
            n => format!("{n} staged"),
        };
        let e = entry(
            C::StashDialog(StashScope::Staged),
            "Stash staged changes…",
            "save shelve park index staged only partial stash push message",
        );
        out.push(with_detail(e, Some(staged)));
        let n = wip_files(snap).len();
        let files = format!("{n} file{}", if n == 1 { "" } else { "s" });
        let e = entry(
            C::StashDialog(StashScope::Files),
            "Stash selected files…",
            "save shelve park some chosen pick files paths partial stash push message",
        );
        out.push(with_detail(e, Some(files)));
    }
    if let Some(top) = snap.stashes.first() {
        out.push(with_detail(entry(C::PopStash, "Pop stash", "unstash apply restore stash"), Some(top.message.clone())));
    }
    if !snap.stashes.is_empty() {
        let len = snap.stashes.len();
        let n = format!("{len} stash{}", if len == 1 { "" } else { "es" });
        let choose = [
            (StashAction::Apply, "Apply stash…", "unstash restore keep stash"),
            (StashAction::Pop, "Pop stash…", "unstash apply restore remove stash"),
            (StashAction::Drop, "Delete stash…", "drop remove discard clear stash"),
        ];
        for (action, label, keywords) in choose {
            out.push(with_detail(entry(C::ChooseStash(action), label, keywords), Some(n.clone())));
        }
        out.push(with_detail(entry(C::ChooseStashToRename, "Rename stash…", "edit reword message describe name stash"), Some(n)));
    }
}

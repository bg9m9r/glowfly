//! The menu of a picked row while several files of its list are picked (right-click, or
//! Shift+F10 on the current row): its items name the count and act on every pick. Unstaged
//! Files: Stage and Stash, then Discard changes and Delete (danger style, last). Staged Files:
//! Unstage and Stash, then Delete. An item a pick blocks is disabled with the reason
//! (`PickBlocks`).

use crate::common::{menu_danger, menu_item};
use eframe::egui;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;
use glowfly_model::msg::{Msg, StageMsg};
use glowfly_model::state::files::{delete_gone, delete_paths_msg, delete_unsafe, discard_paths_msg, discard_unsafe};

/// Why each action of the selection menu is off, if it is: the first pick that blocks it.
#[derive(Default)]
pub(super) struct PickBlocks {
    pub discard: Option<String>,
    pub delete: Option<String>,
    pub stash: Option<String>,
}

impl PickBlocks {
    /// The reasons for the picked `paths`: a name Glowfly can't act on safely blocks them all,
    /// a file gone from the working tree blocks Delete, and a conflicted file Stash.
    pub fn of(t: Lang, snap: &Snapshot, paths: &[String]) -> Self {
        let picked = |f: &&glowfly_core::repo::FileEntry| paths.contains(&f.path);
        let reason = |f: &glowfly_core::repo::FileEntry| f.hazard.as_ref().map(|h| (f.path.clone(), h.reason(t)));
        let hazard = snap.unstaged.iter().chain(&snap.staged).filter(picked).find_map(reason);
        let discard = snap.unstaged.iter().filter(picked).find_map(reason).map(|(p, r)| discard_unsafe(t, &p, &r));
        let delete = match &hazard {
            Some((path, reason)) => Some(delete_unsafe(t, path, reason)),
            None => paths.iter().find(|p| snap.missing_from_workdir(p)).map(|p| delete_gone(t, p)),
        };
        let stash = super::file_menu::stash_picks_blocked(t, snap, paths);
        Self { discard, delete, stash }
    }
}

/// The selection menu for the picked `paths` (in on-screen order) of Unstaged (`staged: false`)
/// or Staged Files; `why` turns items off.
pub(super) fn menu(ui: &mut egui::Ui, t: Lang, msgs: &mut Vec<Msg>, staged: bool, paths: &[String], why: &PickBlocks) {
    let n = paths.len();
    if staged {
        menu_item(msgs, ui, t.trn("details.wip.picks.unstage.item", n, &[]), Msg::Stage(StageMsg::UnstagePaths(paths.to_vec())));
    } else {
        menu_item(msgs, ui, t.trn("details.wip.picks.stage.item", n, &[]), Msg::Stage(StageMsg::StagePaths(paths.to_vec())));
    }
    super::file_menu::stash_picks_item(ui, t, msgs, paths, why.stash.as_deref());
    crate::widgets::menu::separator(ui);
    if !staged {
        let discard = discard_paths_msg(paths.to_vec());
        menu_danger(msgs, ui, t.trn("details.wip.picks.discard.item", n, &[]), why.discard.as_deref(), discard);
    }
    let delete = delete_paths_msg(paths.to_vec());
    menu_danger(msgs, ui, t.trn("details.wip.picks.delete.item", n, &[]), why.delete.as_deref(), delete);
}

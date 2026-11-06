//! The menu of a picked row while several files of its list are picked (right-click, or
//! Shift+F10 on the current row): its items name the count and act on every pick. Unstaged
//! Files: Stage, then Discard changes (danger style, last; disabled with the reason while a
//! pick is a name Glowfly can't act on safely). Staged Files: Unstage.

use crate::common::{menu_danger, menu_item};
use eframe::egui;
use glowfly_core::i18n::Lang;
use glowfly_model::msg::{Msg, StageMsg};

/// The selection menu for the picked `paths` (in on-screen order) of Unstaged (`staged: false`)
/// or Staged Files; `unsafe_reason` turns the discard off.
pub(super) fn menu(ui: &mut egui::Ui, t: Lang, msgs: &mut Vec<Msg>, staged: bool, paths: &[String], unsafe_reason: Option<&str>) {
    let n = paths.len();
    if staged {
        menu_item(msgs, ui, t.trn("details.wip.picks.unstage.item", n, &[]), Msg::Stage(StageMsg::UnstagePaths(paths.to_vec())));
        return;
    }
    menu_item(msgs, ui, t.trn("details.wip.picks.stage.item", n, &[]), Msg::Stage(StageMsg::StagePaths(paths.to_vec())));
    crate::widgets::menu::separator(ui);
    let discard = glowfly_model::state::files::discard_paths_msg(paths.to_vec());
    menu_danger(msgs, ui, t.trn("details.wip.picks.discard.item", n, &[]), unsafe_reason, discard);
}

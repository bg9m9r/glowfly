//! The Unstaged and Staged Files lists' keyboard focus (`list_focus.rs`): one stable focus
//! target per list, its keys taken by the shortcuts before any binding while it has the focus,
//! the list `update` asked to give the focus to, and whether a list has it
//! (`ui/shortcuts/global.rs` doesn't count it as typing).

use crate::keymap::KeyAction;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::list_keys::KeySet;
use eframe::egui;

/// The focus target of the Unstaged (`staged: false`) or Staged Files list.
pub(crate) fn list_id(staged: bool) -> egui::Id {
    egui::Id::new(("wip-list-focus", staged))
}

/// Which WIP file list has the keyboard focus, if one does (`Some(staged)`).
pub(crate) fn focused_list(ctx: &egui::Context) -> Option<bool> {
    let focused = ctx.memory(|m| m.focused())?;
    [false, true].into_iter().find(|&staged| list_id(staged) == focused)
}

/// Registers the list's focus target over `rect` (its rows, or its "none" line), named `name`,
/// with `page` rows in view.
pub(super) fn target(ui: &mut egui::Ui, staged: bool, rect: egui::Rect, name: &str, page: usize) -> egui::Response {
    super::list_focus::target(ui, list_id(staged), rect, name, page)
}

/// This frame's keys of the focused list as messages, before the bindings see them; nothing
/// while a dialog, Preferences, the palette or a credential prompt is `covered` over it.
pub(crate) fn input(ctx: &egui::Context, covered: bool, msgs: &mut Vec<Msg>) {
    super::list_focus::restore(ctx);
    let Some(staged) = focused_list(ctx).filter(|_| !covered) else { return };
    for key in super::list_focus::take(ctx, list_id(staged), KeySet::ALL) {
        msgs.push(Msg::Shortcut(KeyAction::WipList { staged, key }));
    }
}

/// Gives the list the keyboard focus (a click on one of its rows).
pub(super) fn request(ctx: &egui::Context, staged: bool) {
    super::list_focus::request(ctx, list_id(staged));
}

/// Gives the focus to the list `update` asked for (Left or Right went to it).
pub(super) fn requested(ctx: &egui::Context, st: &mut AppState) {
    if let Some(staged) = st.view.wip_focus.take() {
        request(ctx, staged);
    }
}

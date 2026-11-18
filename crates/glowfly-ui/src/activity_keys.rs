//! The keyboard focus of the activity list on `list_focus.rs`: the rows are one Tab stop (a
//! focus target over them), a click on a row gives the list the focus, and while it has it
//! Up/Down, Page Up/Page Down, Home/End and Enter/Space reach update as `LogsMsg::RowKey`, taken
//! before any binding and not while a dialog, Preferences, the palette or a credential prompt
//! covers the window. Keys typed in a text field never get here: only the list's own target has
//! the keys.

use super::activity_rows::ROW_H;
use eframe::egui;
use glowfly_model::logs::LogsMsg;
use glowfly_model::msg::Msg;
use glowfly_model::state::list_keys::KeySet;

/// The list's focus target.
pub(crate) fn list_id() -> egui::Id {
    egui::Id::new("activity-list-focus")
}

/// Whether the activity list has the keyboard focus.
pub(crate) fn focused(ctx: &egui::Context) -> bool {
    ctx.memory(|m| m.has_focus(list_id()))
}

/// This frame's keys of the focused list as messages; nothing while `covered`.
pub(crate) fn input(ctx: &egui::Context, covered: bool, msgs: &mut Vec<Msg>) {
    if covered || !focused(ctx) {
        return;
    }
    let keys = KeySet { toggle: true, open: true, ..KeySet::MOVES };
    for key in super::list_focus::take(ctx, list_id(), keys) {
        msgs.push(Msg::Logs(LogsMsg::RowKey(key)));
    }
}

/// Registers the focus target over the list's room (`rows` rows are about to be drawn), named
/// `name`; register it before the scroll area. Whether the list has the focus.
pub(super) fn target(ui: &mut egui::Ui, rows: usize, name: &str) -> bool {
    let rect = ui.available_rect_before_wrap();
    // A page keeps one row of the last in view.
    let page = ((rect.height() / ROW_H).floor() as usize).saturating_sub(1);
    super::list_focus::target(ui, list_id(), rect, name, page, rows > 0).has_focus()
}

/// Gives the list the keyboard focus (a click on one of its rows).
pub(super) fn request(ctx: &egui::Context) {
    super::list_focus::request(ctx, list_id());
}

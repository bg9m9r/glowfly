//! The Unstaged and Staged Files lists' keyboard focus: one stable focus target per list,
//! Up/Down while it has the focus, and whether a list has it (`ui/shortcuts/global.rs` doesn't
//! count it as typing).

use crate::keymap::KeyAction;
use crate::msg::Msg;
use eframe::egui::{self, Event, EventFilter, Key, WidgetInfo, WidgetType};

/// The focus target of the Unstaged (`staged: false`) or Staged Files list.
pub(crate) fn list_id(staged: bool) -> egui::Id {
    egui::Id::new(("wip-list-focus", staged))
}

/// Which WIP file list has the keyboard focus, if one does (`Some(staged)`).
pub(crate) fn focused_list(ctx: &egui::Context) -> Option<bool> {
    let focused = ctx.memory(|m| m.focused())?;
    [false, true].into_iter().find(|&staged| list_id(staged) == focused)
}

/// Registers the list's focus target over `rect` (its rows, or its "none" line), named `name`.
/// Registered before the rows: Tab reaches it after the header's buttons and before the rows'
/// own, and as it senses no clicks a press on a row goes to the row. While it has the focus the
/// arrows are kept from egui's focus moves, for `arrows`.
pub(super) fn target(ui: &mut egui::Ui, staged: bool, rect: egui::Rect, name: &str) -> egui::Response {
    let r = ui.interact(rect, list_id(staged), egui::Sense::focusable_noninteractive());
    r.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, name));
    if r.has_focus() {
        let filter = EventFilter { vertical_arrows: true, horizontal_arrows: true, tab: false, escape: false };
        ui.memory_mut(|m| m.set_focus_lock_filter(r.id, filter));
    }
    r
}

/// While the list has the focus, turns this frame's plain Up and Down presses (repeats
/// included) into steps between its files. Arrows with Shift, Ctrl, Alt or Cmd are left alone.
pub(super) fn arrows(ui: &egui::Ui, staged: bool, msgs: &mut Vec<Msg>) {
    if !ui.memory(|m| m.has_focus(list_id(staged))) {
        return;
    }
    ui.ctx().input_mut(|i| {
        let mut n = 0;
        while n < i.events.len() {
            let down = match &i.events[n] {
                Event::Key { key: key @ (Key::ArrowUp | Key::ArrowDown), pressed: true, modifiers, .. } if modifiers.is_none() => {
                    Some(*key == Key::ArrowDown)
                }
                _ => None,
            };
            match down {
                Some(down) => {
                    msgs.push(Msg::Shortcut(KeyAction::StepFile { staged, down }));
                    i.events.remove(n);
                }
                None => n += 1,
            }
        }
    });
}

/// Gives the list the keyboard focus (a click on one of its rows).
pub(super) fn request(ctx: &egui::Context, staged: bool) {
    ctx.memory_mut(|m| m.request_focus(list_id(staged)));
}

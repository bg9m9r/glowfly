//! The keyboard in every menu (menu spec §9). `menu::styled` draws each menu's contents through
//! `contents`, one level per menu layer; a submenu's level is drawn inside its parent's contents.
//!
//! - In a level (`level.rs`): Up and Down between rows, wrapping, skipping headers and
//!   separators (they can't take focus); Home and End; Right and Left between a row's parts
//!   (a split row's preview button, a repository row's star and ×); typeahead on the labels
//!   in the active language; Enter and Space click the focused item (egui's own). Blocked rows
//!   take the focus, as they explain themselves.
//! - Submenus: Right, Enter or Space on a submenu row opens it with its first row focused
//!   (`widgets/menu_submenu_keyed.rs`, since egui opens its own only under the pointer); Left or
//!   Esc closes one level; Esc in the root menu closes it and gives the focus back to the row or
//!   button that opened it.
//! - Opening: a menu that opens from the keyboard (Enter, Space or Alt+Down on a dropdown
//!   button, Shift+F10 on a row) focuses its first item; with the mouse, Down or Up does.
//! - Focus stays inside: focus that leaves the open menu for a widget behind it is put back
//!   (`keep_inside`), and the open menu's keys are kept from everything behind it (`stash.rs`).
//!
//! The items are found after drawing (`items.rs`). What is kept between frames is in
//! `memory.rs`. egui reports no Menu (Apps) key, so Shift+F10 alone opens a row's menu.

mod items;
mod level;
mod memory;
mod stash;

#[cfg(test)]
mod bar_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod view_tests;

pub(crate) use items::register;
pub(crate) use stash::{capture, take_key};

use eframe::egui::{self, Id, Key, LayerId, Modifiers, Order, Pos2, Rect, Ui};

/// What a menu level is, which decides what Esc and Left do in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// One of egui's menus (a context menu, a dropdown, the menu bar's): Esc closes it.
    Root,
    /// A menu in egui's popup: Esc closes it, and so do Enter and Space on an item when a click
    /// would (`close_on_click`).
    Popup { close_on_click: bool },
    /// egui's own submenu of the row `row`, opened by the pointer: Left and Esc close it while
    /// the pointer is on the row; else Left goes back to the row and Esc closes the whole menu.
    Native { row: Id },
    /// A submenu of the row `row` opened from the keyboard: Left and Esc close it.
    Keyed { row: Id },
}

/// Draws a menu level's contents with `add`, then handles its keys.
pub(crate) fn contents<R>(ui: &mut Ui, kind: Kind, add: impl FnOnce(&mut Ui) -> R) -> R {
    let ctx = ui.ctx().clone();
    let root = memory::enter(&ctx);
    if root {
        stash::restore(&ctx);
        keep_inside(&ctx);
        // A key may close the menu (an item's click, Esc): the next frame draws it closed.
        if ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, .. }))) {
            ctx.request_repaint();
        }
    }
    // What opened it, read before its contents name the openers of their own submenus.
    let opener = memory::opener(&ctx).or(ctx.memory(|m| m.focused()));
    let start = items::start(ui);
    let r = add(ui);
    let rows = items::rows(ui, start);
    level::keys(ui, kind, &rows, opener);
    if memory::leave(&ctx) {
        stash::keep(&ctx);
    }
    r
}

/// Names the widget the menu about to be drawn opens from (its button, a focused row), for
/// the focus to go back to when Esc closes it. Call before showing the menu.
pub(crate) fn opener(ctx: &egui::Context, id: Option<Id>) {
    memory::set_opener(ctx, id);
}

/// Has the first item of the level drawn on `layer` take the focus, once it can.
pub(crate) fn request_first(ctx: &egui::Context, layer: LayerId) {
    let mut nav = memory::load(ctx);
    nav.pending.push((ctx.cumulative_pass_nr(), layer));
    memory::store(ctx, nav);
}

/// Has the first item of the menu `ui` is drawing take the focus, once it can.
pub(crate) fn focus_first(ui: &Ui) {
    request_first(ui.ctx(), ui.layer_id());
}

/// Leaves Enter, Space and Right on the submenu rows of the menu `ui` is drawing to the
/// caller this frame (the title bar's list of the five menus shows a menu alone instead).
pub(crate) fn caller_opens(ui: &Ui) {
    let mut nav = memory::load(ui.ctx());
    nav.caller_opens.push((ui.ctx().cumulative_pass_nr(), ui.layer_id()));
    memory::store(ui.ctx(), nav);
}

/// Whether `caller_opens` was called for `ui`'s menu this frame.
pub(crate) fn caller_opens_set(ui: &Ui) -> bool {
    let pass = ui.ctx().cumulative_pass_nr();
    memory::load(ui.ctx()).caller_opens.contains(&(pass, ui.layer_id()))
}

/// Whether Left or Esc in the submenu on `layer` asked it to close this frame: `Some(true)` for
/// Esc.
pub(crate) fn take_close(ctx: &egui::Context, layer: LayerId) -> Option<bool> {
    let mut nav = memory::load(ctx);
    let at = nav.close.iter().position(|&(l, _)| l == layer)?;
    let (_, esc) = nav.close.remove(at);
    memory::store(ctx, nav);
    Some(esc)
}

/// A submenu's row closed the whole menu from the keyboard: the focus goes back to what opened
/// the root menu, as Esc in it does.
pub(crate) fn closed_by_key(ctx: &egui::Context) {
    let mut nav = memory::load(ctx);
    nav.closed_pass = nav.pass;
    let back = memory::root(&nav).and_then(|l| l.invoker);
    memory::store(ctx, nav);
    if let Some(back) = back {
        level::focus(ctx, back);
    }
}

/// Notes where a keyboard-opened submenu was drawn, so a click in it doesn't count as a click
/// outside its menu (`pointer_in_keyed`).
pub(crate) fn note_keyed(ctx: &egui::Context, rect: Rect) {
    let mut nav = memory::load(ctx);
    nav.keyed.push((ctx.cumulative_pass_nr(), rect));
    memory::store(ctx, nav);
}

/// Where the pointer is pressed or clicked inside a keyboard-opened submenu drawn last frame or
/// this one. egui's menus close on a press outside their own rects, which don't include it.
pub(crate) fn pointer_in_keyed(ctx: &egui::Context) -> Option<Pos2> {
    let pos = ctx.input(|i| i.pointer.interact_pos())?;
    let nav = memory::load(ctx);
    nav.keyed.iter().any(|&(_, r)| r.contains(pos)).then_some(pos)
}

/// Whether Shift+F10 was pressed this frame; taken when it was.
pub(crate) fn take_menu_key(ctx: &egui::Context) -> bool {
    ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::F10))
}

/// Whether Alt+Down was pressed this frame; taken when it was.
pub(crate) fn take_alt_down(ctx: &egui::Context) -> bool {
    ctx.input_mut(|i| i.consume_key(Modifiers::ALT, Key::ArrowDown))
}

/// Puts the focus back in the open menu when it moved out to a widget behind it (the bar, a
/// view). Runs as a root menu's contents start; code that draws a menu's opener before it can
/// call it first while it knows the menu is open.
pub(crate) fn keep_inside(ctx: &egui::Context) {
    let nav = memory::load(ctx);
    let back = nav.prev.iter().chain(&nav.levels).rev().find_map(|l| l.last_focus);
    let Some(back) = back.filter(|_| memory::open(&nav, nav.pass)) else { return };
    let Some(f) = ctx.memory(|m| m.focused()).filter(|&f| f != back) else { return };
    if level::layer_of(ctx, f).is_some_and(|l| l.order != Order::Foreground) {
        level::focus(ctx, back);
    }
}

/// Whether Left and Right may switch the bar's menus: the focus is in the root menu, not a
/// submenu, or nowhere (a text field elsewhere keeps them).
pub(crate) fn may_step(ctx: &egui::Context) -> bool {
    let nav = memory::load(ctx);
    let root = memory::root(&nav).map(|l| l.layer);
    match ctx.memory(|m| m.focused()) {
        None => true,
        Some(f) => root.is_some() && level::layer_of(ctx, f) == root,
    }
}

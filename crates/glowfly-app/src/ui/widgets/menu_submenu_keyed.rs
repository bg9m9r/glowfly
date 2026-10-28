//! A submenu opened from the keyboard (`menu_submenu::show`). egui 0.31 opens its own submenu
//! only while the pointer hovers the row's button, and keeps that state private, so Right,
//! Enter or Space on a focused submenu row opens one of these instead: an egui menu root of
//! its own (`MenuRoot`, drawn like any of egui's menus, so its items close it and its own
//! submenus work), placed where egui's would go (`menu_submenu::placement`: right of the menu
//! with its first row level with the row, or flipped left). Its first row takes the focus.
//!
//! It closes when Left or Esc in it asks (`menu_keys::take_close`), when one of its items
//! closes the menu (then the parent closes too), when the pointer moves on to another row of
//! the parent, and when its row isn't drawn. A press inside it isn't a press outside the
//! parent menu (`menu_keys::note_keyed`).

use super::menu_submenu::placement;
use crate::ui::menu_keys;
use eframe::egui::menu::MenuRoot;
use eframe::egui::{self, Frame, Id, LayerId, Order, Rect, Response, Ui};

fn key(row: Id) -> Id {
    row.with("keyed")
}

/// The layer the keyboard-opened submenu of the row button `button` is drawn on.
pub(super) fn layer(button: Id) -> LayerId {
    LayerId::new(Order::Foreground, button.with("__menu"))
}

/// Whether the row `row`'s keyboard-opened submenu is open (drawn last frame or this one).
pub(super) fn is_open(ctx: &egui::Context, row: Id) -> bool {
    let pass = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<(u64, MenuRoot)>(key(row))).is_some_and(|(p, _)| p + 1 >= pass)
}

/// Opens the row `row`'s submenu for its button `button`; it shows from this frame.
pub(super) fn open(ctx: &egui::Context, row: Id, button: Id, at: Rect) {
    let pass = ctx.cumulative_pass_nr();
    let root = MenuRoot::new(at.right_top(), button);
    ctx.data_mut(|d| d.insert_temp(key(row), (pass, root)));
    menu_keys::request_first(ctx, layer(button));
}

pub(super) fn close(ctx: &egui::Context, row: Id) {
    ctx.data_mut(|d| d.remove::<(u64, MenuRoot)>(key(row)));
    ctx.request_repaint();
}

/// Draws the row `row`'s keyboard-opened submenu, if open, beside `menu` (the parent menu's
/// rect) for the row's button `button`, with `add`. `parent` is the row's `Ui`, which closes the
/// parent menu when an item closes this one.
pub(super) fn show<R>(parent: &mut Ui, row: Id, button: &Response, menu: Option<Rect>, add: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    let ctx = parent.ctx().clone();
    let pass = ctx.cumulative_pass_nr();
    let (shown, root) = ctx.data(|d| d.get_temp::<(u64, MenuRoot)>(key(row)))?;
    if shown + 1 < pass || root.id != button.id {
        close(&ctx, row);
        return None;
    }
    if let Some(m) = menu {
        let style = ctx.style();
        let margin_top = Frame::menu(&style).total_margin().top;
        let gap = style.spacing.menu_spacing.max(0.0);
        let mut ms = root.menu_state.write();
        let size = ms.rect.size();
        ms.rect = Rect::from_min_size(placement(m, button.rect.top() - margin_top, size, ctx.screen_rect(), gap), size);
    }
    let (response, inner) = root.show(button, add);
    if let Some(r) = ctx.memory(|m| m.area_rect(layer(button.id).id)) {
        menu_keys::note_keyed(&ctx, r);
    }
    let item_closed = root.menu_state.read().response.is_close();
    if item_closed {
        close(&ctx, row);
        parent.close_menu();
    } else if response.is_close() || menu_keys::take_close(&ctx, layer(button.id)).is_some() {
        close(&ctx, row);
    } else {
        ctx.data_mut(|d| d.insert_temp(key(row), (pass, root)));
    }
    inner.map(|r| r.inner)
}

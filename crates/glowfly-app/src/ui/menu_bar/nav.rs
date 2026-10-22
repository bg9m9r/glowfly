//! The keyboard inside an open menu. egui moves focus with the arrows to the nearest widget in
//! that direction on any layer (a toolbar button behind a menu can be nearer than the next
//! item), so here the focused item keeps the arrows from egui and Up and Down step through the
//! menu's own items, stopping at the first and last; Left and Right are left to the bar, which
//! switches menus with them (`may_step`). The first item takes focus when a menu opens from
//! the keyboard or Down is pressed with nothing focused, focus that got out some other way is
//! put back (`keep_inside`), and Enter or Space on the focused item clicks it (egui's own). What
//! it keeps between frames lives in egui's temp data.

use crate::state::AppState;
use eframe::egui::{self, Event, EventFilter, Id, Key, LayerId, Modifiers, Order, Ui};

#[derive(Clone, Copy, Debug, Default)]
struct NavMemory {
    /// The layer of the menu drawn last.
    layer: Option<LayerId>,
    /// The last widget of that menu that had the focus.
    last_inside: Option<Id>,
}

const NAV: &str = "menu-nav";

fn load(ctx: &egui::Context) -> NavMemory {
    ctx.data(|d| d.get_temp(Id::new(NAV)).unwrap_or_default())
}

fn store(ctx: &egui::Context, m: NavMemory) {
    ctx.data_mut(|d| d.insert_temp(Id::new(NAV), m));
}

/// Draws a menu's items with `add`, keeping the keyboard in them.
pub(super) fn contents(ui: &mut Ui, st: &mut AppState, add: impl FnOnce(&mut Ui, &mut AppState)) {
    let ctx = ui.ctx().clone();
    let first = ui.next_auto_id();
    let start = salt_of(ui, first);
    let layer = ui.layer_id();
    // A menu opened with the mouse: Down focuses its first item (taken, so no view behind
    // moves its selection too).
    if ctx.memory(|m| m.focused()).is_none() && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
        st.view.menu_keys.focus_first = true;
    }
    if st.view.menu_keys.focus_first {
        // A menu's first frame only sizes it (its items can't take focus yet), and egui would
        // carry the focus on in an arrow's direction: the next frame does it then.
        if ui.is_sizing_pass() || arrow_pressed(&ctx) {
            ctx.request_repaint();
        } else {
            focus(&ctx, first);
            st.view.menu_keys.focus_first = false;
        }
    }
    add(ui, st);
    let items: Vec<Id> = start
        .map(|s| allocated(s, ui.next_auto_id()))
        .unwrap_or_default()
        .into_iter()
        .filter(|&id| ctx.read_response(id).is_some_and(|r| r.layer_id == layer && r.sense.is_focusable()))
        .collect();
    let mut mem = load(&ctx);
    mem.layer = Some(layer);
    if let Some(f) = ctx.memory(|m| m.focused()).filter(|&f| ctx.read_response(f).is_some_and(|r| r.layer_id == layer)) {
        mem.last_inside = Some(f);
        // egui's own arrow moves would leave the menu; Up and Down are handled below.
        ctx.memory_mut(|m| {
            m.set_focus_lock_filter(f, EventFilter { horizontal_arrows: true, vertical_arrows: true, ..Default::default() })
        });
        let step = ctx.input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                1
            } else if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                -1
            } else {
                0
            }
        });
        let at = items.iter().position(|&x| x == f);
        let to = match (at, step) {
            (_, 0) => None,
            (Some(i), d) => items.get(i.saturating_add_signed(d)).copied(),
            // On the title row above the items: Down goes to the first.
            (None, 1) => items.first().copied(),
            (None, _) => None,
        };
        if let Some(to) = to.filter(|&to| to != f) {
            focus(&ctx, to);
        }
    }
    store(&ctx, mem);
}

/// Focuses `id`, with a frame after it so the item takes the arrows from egui before the next key.
fn focus(ctx: &egui::Context, id: Id) {
    ctx.memory_mut(|m| m.request_focus(id));
    ctx.request_repaint();
}

/// The auto-id counter that gives `ui`'s next widget the id `next`: a child ui counts its
/// widgets' ids up from its unique id, and a menu draws its items into one.
fn salt_of(ui: &Ui, next: Id) -> Option<u64> {
    let base = ui.unique_id().value().wrapping_add(1);
    (0..256u64).map(|j| base.wrapping_add(j)).find(|&s| Id::new(s) == next)
}

/// The ids of the widgets allocated from counter `start` up to the one that would get `next`.
fn allocated(start: u64, next: Id) -> Vec<Id> {
    (0..1024u64).map(|k| Id::new(start.wrapping_add(k))).take_while(|&id| id != next).collect()
}

/// Puts the focus back in the open menu when it moved out to a widget behind (the bar or a
/// view; a submenu is on top too, so it may go there). Call before the bar is drawn.
pub(in crate::ui) fn keep_inside(ctx: &egui::Context) {
    let Some(back) = load(ctx).last_inside.filter(|_| super::any_open(ctx)) else { return };
    let Some(f) = ctx.memory(|m| m.focused()).filter(|&f| f != back) else { return };
    if ctx.read_response(f).is_some_and(|r| r.layer_id.order != Order::Foreground) {
        focus(ctx, back);
    }
}

/// Whether Left and Right may switch menus: the focus is in the menu drawn last, or nowhere
/// (a text field elsewhere keeps them).
pub(in crate::ui) fn may_step(ctx: &egui::Context) -> bool {
    let layer = load(ctx).layer;
    match ctx.memory(|m| m.focused()) {
        None => true,
        Some(f) => ctx.read_response(f).is_some_and(|r| Some(r.layer_id) == layer),
    }
}

/// Forgets the closed menu's focus, so a later one never takes the keyboard back to it.
pub(in crate::ui) fn forget(ctx: &egui::Context) {
    if load(ctx).layer.is_some() {
        store(ctx, NavMemory::default());
    }
}

/// Whether an arrow key was pressed this frame (taken or not).
fn arrow_pressed(ctx: &egui::Context) -> bool {
    ctx.input(|i| {
        i.raw
            .events
            .iter()
            .any(|e| matches!(e, Event::Key { key: Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight, pressed: true, .. }))
    })
}

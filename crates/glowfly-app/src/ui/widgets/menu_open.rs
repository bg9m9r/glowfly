//! Opening egui's menus from a button, with the keyboard too (`menu.rs`'s dropdowns, a submenu
//! button outside a menu, `menu_anchor::menu_below`): a click, or Enter or Space on the focused
//! button (egui's own), Alt+Down on it, and Right on a submenu button in a popup. The button is
//! named as the menu's opener, so Esc gives it the focus back (`menu_keys`), and a press inside
//! a keyboard-opened submenu doesn't close the menu (`guarded`).

use crate::ui::menu_keys;
use eframe::egui::menu::{BarState, MenuRoot};
use eframe::egui::{self, Button, Event, Frame, InnerResponse, Key, Modifiers, Pos2, Rect, Response, Ui, WidgetText};

/// Where egui's click opens `button`'s menu: below it, its first item lined up with the button.
pub(in crate::ui) fn below_pos(button: &Response) -> Pos2 {
    let style = button.ctx.style();
    let mut pos = button.rect.left_bottom();
    pos.x -= Frame::menu(&style).total_margin().left;
    pos.y += style.spacing.menu_spacing;
    pos
}

/// Runs egui's interaction `f` on `bar`, as if the open menu's rect held the pointer when it is
/// pressed inside a keyboard-opened submenu (which egui doesn't know is part of the menu).
pub(in crate::ui) fn guarded(ctx: &egui::Context, bar: &mut BarState, f: impl FnOnce(&mut BarState)) {
    let saved = menu_keys::pointer_in_keyed(ctx).and_then(|p| {
        let root = bar.as_ref()?;
        let mut ms = root.menu_state.write();
        let old = ms.rect;
        ms.rect = old.union(Rect::from_min_max(p, p));
        Some((root.menu_state.clone(), old))
    });
    f(bar);
    if let Some((ms, old)) = saved {
        ms.write().rect = old;
    }
}

/// `button`'s menu kept in `bar`, drawn by `add` while open: opened by a click, by Enter or
/// Space on the focused button, by Alt+Down on it, and with `right_opens` by Right.
pub(in crate::ui) fn dropdown<R>(
    bar: &mut BarState,
    button: &Response,
    right_opens: bool,
    add: impl FnOnce(&mut Ui) -> R,
) -> Option<InnerResponse<R>> {
    let ctx = &button.ctx;
    menu_keys::capture(ctx);
    let open = bar.as_ref().is_some_and(|r| r.id == button.id);
    if button.has_focus() && !open {
        let right = right_opens && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowRight));
        if right || menu_keys::take_alt_down(ctx) {
            ***bar = Some(MenuRoot::new(below_pos(button), button.id));
        }
    }
    menu_keys::opener(ctx, Some(button.id));
    // egui closes the menu on Esc before drawing it; its contents close it instead, one level
    // at a time (a dropdown inside a popup's menu closes alone).
    let esc: Vec<Event> = if open { ctx.input_mut(|i| i.events.extract_if(.., is_esc).collect()) } else { Vec::new() };
    guarded(ctx, bar, |bar| MenuRoot::stationary_click_interaction(button, bar));
    ctx.input_mut(|i| i.events.extend(esc));
    bar.show(button, add)
}

fn is_esc(e: &mut Event) -> bool {
    matches!(e, Event::Key { key: Key::Escape, pressed: true, .. })
}

/// A button titled `title` with its menu below it, kept under `ui`'s id as egui's
/// `menu_button` does: `dropdown`.
pub(in crate::ui) fn titled<R>(
    ui: &mut Ui,
    title: impl Into<WidgetText>,
    right_opens: bool,
    add: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<Option<R>> {
    custom(ui, Button::new(title), right_opens, add)
}

/// `titled` with a button of the caller's.
pub(in crate::ui) fn custom<R>(
    ui: &mut Ui,
    button: Button<'_>,
    right_opens: bool,
    add: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<Option<R>> {
    let bar_id = ui.id();
    let mut bar = BarState::load(ui.ctx(), bar_id);
    let resp = ui.add(button);
    let inner = dropdown(&mut bar, &resp, right_opens, add);
    bar.store(ui.ctx(), bar_id);
    InnerResponse::new(inner.map(|r| r.inner), resp)
}

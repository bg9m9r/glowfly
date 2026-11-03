//! The popover behind `menu::popover_below`: a popup frame of a set width below its anchor,
//! for contents that are not menu rows (the conflict check's report).
//!
//! egui's own `popup_below_widget` lays its contents out top-down *justified*, and a justified
//! layout makes every wrapping label justify its rows: a row cut short by a long, unbreakable
//! word (a release bot's branch name) got its glyphs spread across the width ("C o n f l i c t s
//! w i t h", issue #523). It also takes its width from the anchor and opens at the anchor's left
//! edge, so near the window's right edge it was only pushed back until it touched it. This one
//! lays its contents out top-down, left-aligned and unjustified, in a frame `width` wide (narrower
//! only in a narrower window) with `PAD` of padding, right-aligned with its anchor and kept
//! `menu_submenu::EDGE` inside the window.

use super::menu_submenu::EDGE;
use eframe::egui::{
    self, Align, Area, Color32, Frame, Id, Key, Layout, Margin, Order, PopupCloseBehavior, Pos2, Rect, Response, Shadow, Stroke, Ui,
    UiBuilder, UiKind,
};

/// The gap between the anchor's bottom and the popover's frame.
const GAP: f32 = 4.0;

/// The padding inside the frame, on every side.
pub(crate) const PAD: i8 = 16;

/// Draws the popover `id`, `width` wide with its frame, while it is open (opening it is the caller's) and closes it on Esc or
/// as `close` says.
pub(super) fn below<R>(
    ui: &Ui,
    id: Id,
    anchor: &Response,
    width: f32,
    close: PopupCloseBehavior,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    if !ui.memory(|m| m.is_popup_open(id)) {
        return None;
    }
    let mut a = anchor.rect;
    if let Some(to_global) = ui.ctx().layer_transform_to_global(ui.layer_id()) {
        a = to_global * a;
    }
    let frame = Frame::popup(ui.style()).inner_margin(egui::Margin::same(PAD));
    let screen = ui.ctx().screen_rect().shrink(EDGE);
    let outer_w = width.min(screen.width());
    let inner_w = (outer_w - frame.total_margin().sum().x).max(0.0);
    let pos = place(a, outer_w, screen);
    register(ui, id, anchor);
    let response = Area::new(id.with("popover")).kind(UiKind::Popup).order(Order::Foreground).fixed_pos(pos).constrain_to(screen).show(
        ui.ctx(),
        |ui| {
            frame
                .show(ui, |ui| {
                    ui.set_width(inner_w);
                    ui.with_layout(Layout::top_down(Align::LEFT), add_contents).inner
                })
                .inner
        },
    );
    let should_close = match close {
        PopupCloseBehavior::CloseOnClick => anchor.clicked_elsewhere(),
        PopupCloseBehavior::CloseOnClickOutside => anchor.clicked_elsewhere() && response.response.clicked_elsewhere(),
        PopupCloseBehavior::IgnoreClicks => false,
    };
    if ui.input(|i| i.key_pressed(Key::Escape)) || should_close {
        ui.memory_mut(|m| m.close_popup());
    }
    Some(response.inner)
}

/// Registers the popover as an open popup of `ui`'s layer, as egui's own popups do, so that
/// layer's tooltips (the toolbar's) stay hidden while it is open; egui keeps that set to itself,
/// so this draws egui's popup for `id` empty and invisible: no fill, edge, shadow or margin, the
/// anchor's width and no height. It closes on Esc as the popover does, and ignores clicks.
fn register(ui: &Ui, id: Id, anchor: &Response) {
    let mut style = (**ui.style()).clone();
    style.visuals.window_fill = Color32::TRANSPARENT;
    style.visuals.window_stroke = Stroke::NONE;
    style.visuals.popup_shadow = Shadow::NONE;
    style.spacing.menu_margin = Margin::ZERO;
    let hidden = Ui::new(ui.ctx().clone(), id.with("register"), UiBuilder::new().layer_id(ui.layer_id()).style(style));
    egui::popup::popup_below_widget(&hidden, id, anchor, PopupCloseBehavior::IgnoreClicks, |_| {});
}

/// The popover's top-left corner, `outer_w` wide, below `anchor`: right-aligned with it, then
/// moved only as far as it takes to stay inside `screen`.
fn place(anchor: Rect, outer_w: f32, screen: Rect) -> Pos2 {
    let x = (anchor.right() - outer_w).min(screen.right() - outer_w).max(screen.left());
    Pos2::new(x, anchor.bottom() + GAP)
}

#[cfg(test)]
#[path = "popover_tests.rs"]
mod tests;

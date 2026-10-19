//! The diff header: one 48 px row on the raised surface, a 1 px border under it. On the left the
//! file (its type icon, folder and name, a status chip and a context chip); on the right, in
//! reading order and split by thin dividers, previous / next change with a counter, the View
//! menu, the file actions and the diff tool, Stage file or Unstage file, and Close. The row
//! never wraps: `fit::plan` decides which parts give way when it is short of room. The parts
//! are drawn from `state::diff_header::HeaderModel`, built once per frame.

mod actions;
mod controls;
mod fit;
mod identity;
mod nav;
mod view_menu;

#[cfg(test)]
use nav::step_tip;

use crate::keymap::Keymap;
use crate::msg::Msg;
use crate::state::diff_header::HeaderModel;
use crate::ui::theme::{color, spacing};
use eframe::egui::{Align, Layout, Rect, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use fit::Widths;
use trellis_core::i18n::Lang;

/// The header's height.
pub(super) const HEIGHT: f32 = 48.0;

/// Draws the header for `m` across the width of `ui`.
pub(super) fn show(ui: &mut Ui, m: &HeaderModel, keys: &Keymap, t: Lang, msgs: &mut Vec<Msg>) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEIGHT), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 0.0, color::panel());
    let y = rect.bottom() - 0.5;
    p.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], Stroke::new(1.0_f32, color::border()));
    let pad = spacing::PANEL_PAD_F;
    let inner = Rect::from_min_max(pos2(rect.left() + pad, rect.top()), pos2(rect.right() - pad, rect.bottom() - 1.0));
    let w = widths(ui, m);
    let plan = fit::plan(&w, inner.width());
    let right = Rect::from_min_max(pos2((inner.right() - plan.right_width).max(inner.left()), inner.top()), inner.max);
    let left = Rect::from_min_max(inner.min, pos2((right.left() - fit::MIN_GAP).max(inner.left()), inner.bottom()));
    ui.scope_builder(UiBuilder::new().max_rect(left).layout(Layout::left_to_right(Align::Center)), |ui| {
        ui.set_clip_rect(left.intersect(ui.clip_rect()));
        identity::show(ui, m, &w, &plan);
    });
    ui.scope_builder(UiBuilder::new().max_rect(right).layout(Layout::left_to_right(Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        right_groups(ui, m, &w, &plan, keys, t, msgs);
    });
}

/// Every part's width.
fn widths(ui: &Ui, m: &HeaderModel) -> Widths {
    let mut w = Widths { divider: controls::DIVIDER_W, ..Widths::default() };
    identity::widths(ui, m, &mut w);
    (w.nav_arrows, w.counter) = nav::widths(ui, m);
    (w.view_full, w.view_short) = view_menu::widths(ui, m.view.as_ref());
    (w.actions_labelled, w.actions_folded, w.diff_tool) = actions::widths(ui, &m.actions);
    w.primary = actions::primary_width(ui, m);
    w.close = controls::HEIGHT;
    w
}

/// The right-hand groups in reading order, a divider between each two that show.
fn right_groups(ui: &mut Ui, m: &HeaderModel, w: &Widths, plan: &fit::Plan, keys: &Keymap, t: Lang, msgs: &mut Vec<Msg>) {
    let mut first = true;
    let mut divide = |ui: &mut Ui| {
        if !first {
            controls::divider(ui);
        }
        first = false;
    };
    if w.nav_arrows > 0.0 {
        divide(ui);
        nav::show(ui, m, plan.counter, keys, t, msgs);
    }
    if let Some(v) = &m.view {
        divide(ui);
        view_menu::show(ui, v, plan.view_short, msgs);
    }
    if w.actions_labelled + w.diff_tool > 0.0 {
        divide(ui);
        actions::show(ui, m, plan.fold_actions, keys, t, msgs);
    }
    if m.primary.is_some() {
        divide(ui);
        actions::show_primary(ui, m, keys, msgs);
    }
    divide(ui);
    actions::show_close(ui, keys, t, msgs);
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

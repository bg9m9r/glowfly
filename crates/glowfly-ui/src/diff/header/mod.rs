//! The diff header: one 48 px row on the raised surface, a 1 px border under it. From the left:
//! the file (its type icon, folder and name, a status chip and a context chip), 16 px, previous /
//! next change with a counter, a thin divider, an SVG's Text / Preview switch (`svg_switch`), the
//! View menu and ⋯ (the file actions and the diff tool); at the trailing edge Stage file or Unstage file (secondary, small) and Close. The
//! header has no filled button (`docs/COLOUR.md`, "The accent"). The row never wraps: `fit::plan`
//! decides which parts give way when it is short of room. The parts are drawn from
//! `state::diff_header::HeaderModel`, built once per frame.

mod actions;
mod controls;
mod fit;
mod identity;
mod nav;
mod svg_switch;
mod view_menu;

#[cfg(test)]
use nav::step_tip;

use crate::keymap::Keymap;
use crate::msg::Msg;
use crate::state::diff_header::HeaderModel;
use crate::ui::theme::{color, spacing};
use eframe::egui::{Align, Layout, Rect, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use fit::Widths;
use glowfly_core::i18n::Lang;

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
    let w = widths(ui, m, t);
    let plan = fit::plan(&w, inner.width());
    let right = Rect::from_min_max(pos2((inner.right() - plan.right_width).max(inner.left()), inner.top()), inner.max);
    // ⋯ never gives way: the parts before it are clipped short of the room it keeps.
    let more_x = (right.left() - fit::MIN_GAP - w.more).max(inner.left());
    let left = Rect::from_min_max(inner.min, pos2(more_x, inner.bottom()));
    let end = ui
        .scope_builder(UiBuilder::new().max_rect(left).layout(Layout::left_to_right(Align::Center)), |ui| {
            ui.set_clip_rect(left.intersect(ui.clip_rect()));
            identity::show(ui, m, t, &w, &plan);
            leading_tools(ui, m, &w, &plan, keys, t, msgs);
            ui.cursor().left()
        })
        .inner;
    if w.more > 0.0 {
        // Right after View when there is room, else at the edge of the clipped parts.
        let x = end.min(more_x);
        let slot = Rect::from_min_max(pos2(x, inner.top()), pos2(x + w.more, inner.bottom()));
        ui.scope_builder(UiBuilder::new().max_rect(slot).layout(Layout::left_to_right(Align::Center)), |ui| {
            actions::more(ui, m, keys, t, msgs);
        });
    }
    ui.scope_builder(UiBuilder::new().max_rect(right).layout(Layout::left_to_right(Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        if m.primary.is_some() {
            actions::show_primary(ui, m, keys, t, msgs);
            ui.add_space(actions::TRAILING_GAP);
        }
        actions::show_close(ui, keys, t, msgs);
    });
}

/// Every part's width.
fn widths(ui: &Ui, m: &HeaderModel, t: Lang) -> Widths {
    let mut w = Widths { divider: controls::DIVIDER_W, ..Widths::default() };
    identity::widths(ui, m, t, &mut w);
    (w.nav_arrows, w.counter) = nav::widths(ui, m);
    w.svg = svg_switch::width(ui, t, m.svg);
    (w.view_full, w.view_short) = view_menu::widths(ui, t, m.view.as_ref());
    w.more = actions::more_width(&m.actions);
    w.primary = actions::primary_width(ui, t, m);
    w.close = controls::HEIGHT;
    w
}

/// After the file: 16 px and the navigation, then a divider, an SVG's Text / Preview switch and
/// the View menu, and the space before ⋯ (which `show` draws, so it is never clipped).
fn leading_tools(ui: &mut Ui, m: &HeaderModel, w: &Widths, plan: &fit::Plan, keys: &Keymap, t: Lang, msgs: &mut Vec<Msg>) {
    ui.spacing_mut().item_spacing.x = 0.0;
    if w.nav_arrows > 0.0 {
        ui.add_space(fit::NAV_GAP);
        nav::show(ui, m, plan.counter, keys, t, msgs);
    }
    if m.view.is_none() && m.svg.is_none() && w.more == 0.0 {
        return;
    }
    if w.nav_arrows > 0.0 {
        controls::divider(ui);
    } else {
        ui.add_space(fit::NAV_GAP);
    }
    if let Some(on) = m.svg {
        svg_switch::show(ui, t, on, msgs);
        if m.view.is_some() || w.more > 0.0 {
            ui.add_space(fit::TOOLS_SPACE);
        }
    }
    if let Some(v) = &m.view {
        view_menu::show(ui, t, v, plan.view_short, msgs);
        if w.more > 0.0 {
            ui.add_space(fit::TOOLS_SPACE);
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

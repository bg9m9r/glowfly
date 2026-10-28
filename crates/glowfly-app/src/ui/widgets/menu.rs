//! The one menu style, and the only ways to open a menu: every context menu, dropdown,
//! submenu, menu-bar menu and split-button menu calls `style` first, through `context_menu`,
//! `submenu`, `custom_button`, `below`, `popup_below` / `popup_above_or_below` here,
//! `row_menu::RowMenuExt::row_menu` or `menu_anchor::menu_below` (`tests/menus.rs` checks;
//! `popover_below` is the popup for contents that are not menu rows). egui gives its own
//! menus 2 px of button padding and nothing to popups, so the style replaces egui's after it
//! is set: rows `row_h` tall on a `row_h + row_gap` pitch, text `pad_x` inside a transparent
//! pill with `radius::SM` corners, between `min_w` and `max_w` wide (`theme::spacing::menu`,
//! comfortable unless a later setting picks compact in `metrics`).
//!
//! Separators go through `separator`: a 1 px line in the frame's edge colour, 4 px from the
//! rows on each side, never the first child of a menu or right after another one (and a debug
//! build asserts no menu ends with one). Plain lines (headers, notes) use `inset` to line up
//! with the item labels.
//!
//! A destructive item (`menu_reason::ItemState::Danger`, or a `danger_submenu` title) takes
//! `danger_style`: its label in `color::menu_danger` and its hover pill `color::danger_hover`.

use super::super::theme::color;
use super::super::theme::spacing::menu::{self as tokens, Metrics};
use super::super::theme::tokens::radius;
use eframe::egui::menu::BarState;
use eframe::egui::{
    self, AboveOrBelow, Color32, CornerRadius, Id, InnerResponse, PopupCloseBehavior, Response, Stroke, Ui, WidgetText, vec2,
};

/// The row metrics menus use now: comfortable (a compact variant is opt-in, not offered yet).
pub fn metrics(_ui: &Ui) -> Metrics {
    tokens::COMFORTABLE
}

/// The shared menu style. Call it first inside every menu's contents (the openers here do).
pub fn style(ui: &mut Ui) {
    let m = metrics(ui);
    let s = ui.spacing_mut();
    // The height comes from `interact_size`, which buttons and the blocked items grow to.
    s.button_padding = vec2(m.pad_x, 0.0);
    s.interact_size.y = m.row_h;
    s.item_spacing = vec2(tokens::GAP_X, m.row_gap);
    ui.set_min_width(m.min_w);
    // Only ever narrower: setting a wider max would widen the justified rows with it.
    if ui.max_rect().width() > m.max_w {
        ui.set_max_width(m.max_w);
    }
    let v = ui.visuals_mut();
    // Separators take the frame's edge: `border` barely shows on the overlay.
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, v.window_stroke.color);
    let w = &mut v.widgets;
    // What egui's own menu style does, which popups don't get: rows draw no fill or edge at
    // rest, so an enabled item doesn't look pressed.
    w.inactive.weak_bg_fill = Color32::TRANSPARENT;
    w.inactive.bg_stroke = Stroke::NONE;
    w.hovered.bg_stroke = Stroke::NONE;
    w.active.bg_stroke = Stroke::NONE;
    for v in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        v.corner_radius = CornerRadius::same(radius::SM);
    }
}

/// `resp`'s right-click menu at the pointer, in the menu style.
pub fn context_menu(resp: &Response, add_contents: impl FnOnce(&mut Ui)) -> Option<InnerResponse<()>> {
    resp.context_menu(|ui| styled(ui, add_contents))
}

/// A submenu row inside a menu (or, outside one, a button with its dropdown), its contents in
/// the menu style.
pub fn submenu<R>(ui: &mut Ui, title: impl Into<WidgetText>, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<Option<R>> {
    ui.menu_button(title, |ui| styled(ui, add_contents))
}

/// `ui`'s items in the destructive style: the label in the danger colour (`color::menu_danger`,
/// readable on the overlay and the pill) and the hover pill tinted toward it (`color::danger_hover`).
pub fn danger_style(ui: &mut Ui) {
    let v = ui.visuals_mut();
    v.override_text_color = Some(color::menu_danger());
    v.widgets.hovered.weak_bg_fill = color::danger_hover();
    v.widgets.open.weak_bg_fill = color::danger_hover();
}

/// A submenu whose title is destructive (its items are): the title in the danger style, the
/// items in the plain menu style (each destructive one marks itself).
pub fn danger_submenu<R>(ui: &mut Ui, title: impl Into<WidgetText>, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<Option<R>> {
    let plain = ui.visuals().clone();
    ui.scope(|ui| {
        danger_style(ui);
        submenu(ui, title, |ui| {
            *ui.visuals_mut() = plain;
            style(ui);
            add_contents(ui)
        })
    })
    .inner
}

/// A button drawn from `button` that opens its menu below it on a click (a header's ⋯).
pub fn custom_button<R>(ui: &mut Ui, button: egui::Button<'_>, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<Option<R>> {
    egui::menu::menu_custom_button(ui, button, |ui| styled(ui, add_contents))
}

/// A menu below `button` that a click on it opens and closes, kept under the button's own id
/// (a section header's + and ⋯).
pub fn below(ui: &Ui, button: &Response, add_contents: impl FnOnce(&mut Ui)) {
    let key = button.id.with("menu");
    let mut bar = BarState::load(ui.ctx(), key);
    bar.bar_menu(button, |ui| styled(ui, add_contents));
    bar.store(ui.ctx(), key);
}

/// Whether the menu `below` keeps for the button `id` is open.
pub fn below_open(ctx: &egui::Context, id: Id) -> bool {
    let bar = BarState::load(ctx, id.with("menu"));
    bar.as_ref().is_some_and(|root| root.id == id)
}

/// egui's menu bar strip (its menus open through `menu_anchor::menu_below`).
pub fn bar<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    egui::menu::bar(ui, add_contents)
}

/// A menu in egui's popup `id` below `anchor` (a split button's caret, a ⋯ button), in the menu
/// style. Opening and closing it stays the caller's (`toggle_popup`, `close_popup`).
pub fn popup_below<R>(ui: &Ui, id: Id, anchor: &Response, close: PopupCloseBehavior, add_contents: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    popup_above_or_below(ui, id, anchor, AboveOrBelow::Below, close, add_contents)
}

/// `popup_below`, above `anchor` when `side` says so.
pub fn popup_above_or_below<R>(
    ui: &Ui,
    id: Id,
    anchor: &Response,
    side: AboveOrBelow,
    close: PopupCloseBehavior,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    egui::popup::popup_above_or_below_widget(ui, id, anchor, side, close, |ui| styled(ui, add_contents))
}

/// A popover below `anchor`: egui's popup frame around contents that are not a menu's rows (a
/// report, a form), so it keeps the ordinary widget style.
pub fn popover_below<R>(
    ui: &Ui,
    id: Id,
    anchor: &Response,
    close: PopupCloseBehavior,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    egui::popup::popup_below_widget(ui, id, anchor, close, add_contents)
}

/// Runs `add_contents` in the menu style.
pub(in crate::ui) fn styled<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    style(ui);
    let start = (ui.ctx().cumulative_pass_nr(), ui.cursor().top());
    ui.data_mut(|d| d.insert_temp(mark_id(ui, "menu-start"), start));
    let r = add_contents(ui);
    debug_assert!(!just_after_separator(ui), "a menu ends with a separator");
    r
}

/// A menu's separator: a 1 px line across the rows' width, 4 px from the rows on each side.
/// Skipped as the menu's first child and right after another separator, so groups that show
/// nothing don't leave doubled lines. Each menu is its own layer, which keys the marks, so a
/// separator inside a nested scope of the menu counts too.
pub fn separator(ui: &mut Ui) {
    if at_mark(ui, "menu-start") || just_after_separator(ui) {
        return;
    }
    ui.add(egui::Separator::default().spacing(tokens::SEPARATOR_H));
    let mark = (ui.ctx().cumulative_pass_nr(), ui.cursor().top());
    ui.data_mut(|d| d.insert_temp(mark_id(ui, "menu-separator"), mark));
}

/// Whether the last thing drawn in `ui`'s menu this pass is a separator.
fn just_after_separator(ui: &Ui) -> bool {
    at_mark(ui, "menu-separator")
}

/// Whether nothing was drawn in `ui`'s menu since the mark `name` was set this pass.
fn at_mark(ui: &Ui, name: &str) -> bool {
    let mark = ui.data(|d| d.get_temp::<(u64, f32)>(mark_id(ui, name)));
    mark.is_some_and(|(pass, y)| pass == ui.ctx().cumulative_pass_nr() && ui.cursor().top() <= y + 0.5)
}

fn mark_id(ui: &Ui, name: &str) -> Id {
    Id::new((ui.layer_id(), name))
}

/// Lays out `add_contents` at the rows' text inset, for a menu's plain lines (a header, a note)
/// to line up with the item labels.
pub fn inset<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let pad = ui.spacing().button_padding.x as i8;
    egui::Frame::new().inner_margin(egui::Margin { left: pad, right: pad, top: 0, bottom: 0 }).show(ui, add_contents).inner
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::spacing::menu::COMFORTABLE;

    fn in_menu(f: impl FnOnce(&mut Ui) + 'static) {
        let mut f = Some(f);
        let mut h = egui_kittest::Harness::new_ui(move |ui| {
            if let Some(f) = f.take() {
                styled(ui, f);
            }
        });
        h.run();
    }

    #[test]
    fn rows_take_the_comfortable_metrics() {
        in_menu(|ui| {
            let s = ui.spacing();
            assert_eq!(s.button_padding, vec2(COMFORTABLE.pad_x, 0.0));
            assert_eq!(s.interact_size.y, 28.0);
            assert_eq!(s.item_spacing.y, 2.0);
            let a = ui.button("One");
            let b = ui.button("Two");
            assert_eq!(a.rect.height(), 28.0);
            assert_eq!(b.rect.top() - a.rect.top(), 30.0, "a 30 px pitch");
            assert!(ui.min_rect().width() >= COMFORTABLE.min_w);
            assert_eq!(ui.visuals().widgets.inactive.weak_bg_fill, Color32::TRANSPARENT);
        });
    }

    #[test]
    fn separators_are_never_first_or_doubled() {
        in_menu(|ui| {
            let top = ui.cursor().top();
            separator(ui);
            assert_eq!(ui.cursor().top(), top, "no separator as the first child");
            let a = ui.button("One");
            separator(ui);
            separator(ui);
            let b = ui.button("Two");
            // One line: 2 px gap, 4 px separator, 2 px gap.
            assert_eq!(b.rect.top() - a.rect.bottom(), 2.0 + tokens::SEPARATOR_H + 2.0);
        });
    }
}

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
//! rows on each side, never the first child of a menu or right after another one or a group
//! header (and a debug build asserts no menu ends with one). A group's title is `group_header`,
//! the one header style, in place of its separator. Plain lines (object headers, notes) use
//! `inset` to line up with the item labels.
//!
//! Rows lay out on the menu's shared columns (`menu_columns.rs`: state, icon, label, reason,
//! shortcut, chevron or trailing button), which `styled` learns around each menu's contents; a
//! row is `menu_row::item` (re-exported here), which every item helper draws through. A split
//! row adds a trailing button (`menu_split.rs`); names inside labels are cut to fit
//! (`menu_name.rs`).
//!
//! Every menu's contents run through the menu keyboard (`ui/menu_keys`): arrows, Home and End,
//! typeahead, submenus from the keyboard, Esc giving the focus back to what opened the menu.
//! The dropdown openers open on Enter, Space or Alt+Down too (`menu_open.rs`).
//!
//! A destructive item (`menu_reason::ItemState::Danger`, or a `danger_submenu` title) takes
//! `danger_style`: its label in `color::menu_danger` and its hover pill `color::danger_hover`.

use super::super::theme::spacing::menu::{self as tokens, Metrics};
use super::super::theme::tokens::radius;
use super::super::theme::{color, type_scale};
use super::menu_columns::{self as columns, Columns};
pub use super::menu_name::{Named, cut as cut_name, named};
pub use super::menu_row::{Item, item, item_named};
pub use super::menu_split::Part;
use crate::ui::menu_keys::{self, Kind};
use eframe::egui::menu::BarState;
use eframe::egui::{
    self, AboveOrBelow, Color32, CornerRadius, Id, InnerResponse, PopupCloseBehavior, Response, Sense, Stroke, Ui, WidgetText, vec2,
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

/// `resp`'s right-click menu at the pointer, in the menu style; Shift+F10 on the focused widget
/// opens it below the widget.
pub fn context_menu(resp: &Response, add_contents: impl FnOnce(&mut Ui)) -> Option<InnerResponse<()>> {
    super::row_menu::show(resp, false, false, add_contents)
}

/// A submenu row inside a menu (or, outside one, a button with its dropdown), its contents in
/// the menu style. Inside a menu its title starts at the label column and its painted chevron
/// sits in the chevron slot (`menu_columns`), which every row of the menu then keeps. It opens
/// after a short rest on the row and closes after a short grace, on whichever side of the menu
/// it fits (`menu_submenu`).
pub fn submenu<R>(ui: &mut Ui, title: impl Into<WidgetText>, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<Option<R>> {
    if !in_menu(ui) {
        return super::menu_open::titled(ui, title, false, |ui| styled(ui, add_contents));
    }
    let c = columns::note(ui, Columns { chevron: true, ..Columns::default() });
    let pad = ui.spacing().button_padding.x;
    let ink = ui.visuals().override_text_color.unwrap_or_else(|| ui.visuals().widgets.inactive.text_color());
    // egui paints its own arrow in the text colour: the title keeps its colour in the job and
    // the arrow goes clear, for the painted chevron to take its place.
    let mut job = title.into().into_layout_job(ui.style(), egui::FontSelection::Default, egui::Align::Center);
    for s in &mut job.sections {
        if s.format.color == Color32::PLACEHOLDER {
            s.format.color = ink;
        }
    }
    if let Some(first) = job.sections.first_mut() {
        first.leading_space = c.label_x(pad) - pad;
    }
    // Not in a scope: the row keeps the id the menu's keyboard navigation expects of it.
    let plain = ui.visuals().clone();
    ui.visuals_mut().override_text_color = Some(Color32::TRANSPARENT);
    let r = super::menu_submenu::show(ui, WidgetText::LayoutJob(job), |ui, kind| {
        *ui.visuals_mut() = plain.clone();
        styled_as(ui, kind, add_contents)
    });
    ui.visuals_mut().override_text_color = plain.override_text_color;
    // During the open delay egui's button isn't hovered (`menu_submenu`'s cover is), but the
    // row is drawn hovered.
    let lit = ui.rect_contains_pointer(r.response.rect) || r.inner.is_some();
    let at = egui::pos2(r.response.rect.right() - pad - columns::CHEVRON_W / 2.0, r.response.rect.center().y);
    super::chevron(ui.painter(), at, false, if lit { color::text() } else { color::weak() });
    r
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

/// A button drawn from `button` that opens its menu below it on a click, or Enter, Space or
/// Alt+Down while focused (a header's ⋯).
pub fn custom_button<R>(ui: &mut Ui, button: egui::Button<'_>, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<Option<R>> {
    super::menu_open::custom(ui, button, false, |ui| styled(ui, add_contents))
}

/// A menu below `button` that a click on it opens and closes, kept under the button's own id
/// (a section header's + and ⋯).
pub fn below(ui: &Ui, button: &Response, add_contents: impl FnOnce(&mut Ui)) {
    let key = button.id.with("menu");
    let mut bar = BarState::load(ui.ctx(), key);
    super::menu_open::dropdown(&mut bar, button, false, |ui| styled(ui, add_contents));
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

/// `popup_below`, above `anchor` when `side` says so. Alt+Down on the focused anchor opens it
/// too (a click, Enter or Space is the caller's), and a click inside a keyboard-opened submenu
/// of it isn't a click outside it.
pub fn popup_above_or_below<R>(
    ui: &Ui,
    id: Id,
    anchor: &Response,
    side: AboveOrBelow,
    close: PopupCloseBehavior,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let ctx = ui.ctx();
    menu_keys::capture(ctx);
    if anchor.has_focus() && !ctx.memory(|m| m.is_popup_open(id)) && menu_keys::take_alt_down(ctx) {
        ctx.memory_mut(|m| m.open_popup(id));
    }
    menu_keys::opener(ctx, Some(anchor.id));
    let kind = Kind::Popup { close_on_click: matches!(close, PopupCloseBehavior::CloseOnClick) };
    let close = if menu_keys::pointer_in_keyed(ctx).is_some() { PopupCloseBehavior::IgnoreClicks } else { close };
    egui::popup::popup_above_or_below_widget(ui, id, anchor, side, close, |ui| styled_as(ui, kind, add_contents))
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

/// Runs `add_contents` in the menu style, its rows on shared columns (`menu_columns`), with the
/// menu keyboard (`menu_keys`), as one of egui's menus.
pub(in crate::ui) fn styled<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    styled_as(ui, Kind::Root, add_contents)
}

/// `styled` for a menu level of the kind `kind`.
pub(in crate::ui) fn styled_as<R>(ui: &mut Ui, kind: Kind, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    style(ui);
    let start = (ui.ctx().cumulative_pass_nr(), ui.cursor().top());
    ui.data_mut(|d| d.insert_temp(mark_id(ui, "menu-start"), start));
    columns::begin(ui);
    let r = menu_keys::contents(ui, kind, add_contents);
    columns::end(ui);
    debug_assert!(!just_after_separator(ui), "a menu ends with a separator");
    r
}

/// A menu's separator: a 1 px line across the rows' width, 4 px from the rows on each side.
/// Skipped as the menu's first child and right after another separator, so groups that show
/// nothing don't leave doubled lines. Each menu is its own layer, which keys the marks, so a
/// separator inside a nested scope of the menu counts too.
pub fn separator(ui: &mut Ui) {
    if at_mark(ui, "menu-start") || just_after_separator(ui) || at_mark(ui, "menu-header") {
        return;
    }
    ui.add(egui::Separator::default().spacing(tokens::SEPARATOR_H));
    let mark = (ui.ctx().cumulative_pass_nr(), ui.cursor().top());
    ui.data_mut(|d| d.insert_temp(mark_id(ui, "menu-separator"), mark));
}

/// Whether `ui` is inside a menu's contents this pass.
fn in_menu(ui: &Ui) -> bool {
    let start = ui.data(|d| d.get_temp::<(u64, f32)>(mark_id(ui, "menu-start")));
    start.is_some_and(|(pass, _)| pass == ui.ctx().cumulative_pass_nr())
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

/// Lays out `add_contents` at the label column (after the state and icon columns the menu
/// keeps), for a menu's plain lines (an object header, a note) to line up with the item labels.
pub fn inset<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let pad = ui.spacing().button_padding.x;
    let left = columns::used(ui).label_x(pad);
    let m = egui::Margin { left: left.round() as i8, right: pad as i8, top: 0, bottom: 0 };
    egui::Frame::new().inner_margin(m).show(ui, add_contents).inner
}

/// A group's header: small semibold weak text at the label column, in sentence case, 8 px below
/// the row before it (4 as the menu's first child) and 2 above the group's first row. It takes
/// the place of the group's separator (a `separator` right after it draws nothing), can't be
/// focused or clicked, and is a heading to screen readers.
pub fn group_header(ui: &mut Ui, text: &str) -> Response {
    let first = at_mark(ui, "menu-start");
    let gap = if first { tokens::HEADER_FIRST_GAP } else { tokens::HEADER_GAP - ui.spacing().item_spacing.y };
    ui.add_space(gap.max(0.0));
    let pad = ui.spacing().button_padding.x;
    let x = columns::used(ui).label_x(pad);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), type_scale::strong(ui, type_scale::SMALL), color::weak());
    let (rect, resp) = ui.allocate_at_least(vec2(x + galley.size().x + pad, tokens::HEADER_H), Sense::hover());
    if ui.is_rect_visible(rect) {
        let at = egui::pos2(rect.left() + x, rect.center().y - galley.size().y / 2.0);
        ui.painter().galley(at, galley, color::weak());
    }
    // Egui's label node (with its bounds), turned into a named heading.
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
    resp.ctx.accesskit_node_builder(resp.id, |n| {
        n.set_role(egui::accesskit::Role::Heading);
        n.set_label(text);
    });
    let mark = (ui.ctx().cumulative_pass_nr(), ui.cursor().top());
    ui.data_mut(|d| d.insert_temp(mark_id(ui, "menu-header"), mark));
    resp
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

    #[test]
    fn group_headers_sit_8_above_and_2_below() {
        in_menu(|ui| {
            let top = ui.cursor().top();
            group_header(ui, "Favorites");
            let a = ui.button("One");
            assert_eq!(a.rect.top() - top, tokens::HEADER_FIRST_GAP + tokens::HEADER_H + 2.0, "4 above as the first child");
            group_header(ui, "Recent");
            let b = ui.button("Two");
            assert_eq!(b.rect.top() - a.rect.bottom(), tokens::HEADER_GAP + tokens::HEADER_H + 2.0);
        });
    }
}

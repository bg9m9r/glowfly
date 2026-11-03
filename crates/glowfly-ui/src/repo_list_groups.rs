//! The start page's repository groups, nested under Favorites (`repo_list.rs` lays them out):
//! a group's header (its swatch in the rows' swatch column, its name and how many it holds, and
//! at the right Open all, while one of its repositories isn't in a tab, and ⋯), the bar in its
//! colour down its rows, and an empty group's outlined dashed placeholder, which dragging drops
//! onto, with Add repositories… at its right while nothing is dragged. The group menu (from ⋯, a
//! right-click or Shift+F10 on the name): Open all, Add repositories…, Rename…,
//! Color, Reset all group colors (off while every group has its default), Move up and Move
//! down, and Remove group in the danger style.

use super::common::{menu_danger, menu_item, menu_item_if};
use super::repo_list::small_link;
use super::repo_row::{self, Columns};
use super::repo_swatch;
use super::theme::{color, type_scale};
use super::widgets::{self, Icon, menu_columns};
use crate::msg::{GroupMsg, Msg, RecentMsg};
use crate::state::AppState;
use crate::state::repo_groups::{colours_at_default, members};
use crate::state::repo_lists::{is_missing, is_open};
use crate::ui::theme::sizes::ControlSize;
use crate::ui::theme::tokens::radius;
use crate::ui::widgets::menu::{self, separator};
use crate::ui::widgets::row_menu::RowMenuExt;
use eframe::egui::{self, Align, Color32, CornerRadius, Layout, Rect, RichText, Sense, Shape, Stroke, pos2, vec2};

/// A header's height: room for ⋯.
pub(super) const HEADER_H: f32 = 28.0;
/// The bar down a group's rows: its width, and its right edge's distance left of the rows.
const BAR_W: f32 = 3.0;
const BAR_GAP: f32 = 6.0;
/// The placeholder's dashes and gaps.
const DASH: f32 = 4.0;
const DASH_GAP: f32 = 3.0;

/// A header row `HEADER_H` tall: `left` from `inset` (where the rows' names start, or their
/// swatch column), then `right` from the right edge.
pub(super) fn header_row(ui: &mut egui::Ui, inset: f32, left: impl FnOnce(&mut egui::Ui), right: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(vec2(ui.available_width(), HEADER_H), Layout::left_to_right(Align::Center), |ui| {
        ui.add_space(inset);
        left(ui);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // Its last action (a group's ⋯) lines up with the rows' ⋯.
            ui.add_space(repo_row::END_PAD);
            right(ui);
        });
    });
}

/// The header of group `g`, lined up with its rows (`cols`).
pub(super) fn header(ui: &mut egui::Ui, st: &AppState, g: usize, cols: Columns, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let group = &st.settings.repo_groups[g];
    let paths = members(&st.settings, g);
    let unopened = paths.iter().any(|p| !is_open(st, p) && !is_missing(st, p));
    let (icon_x, label_x) = repo_row::lead(ui, cols);
    let mut menu_msgs = Vec::new();
    let left = |ui: &mut egui::Ui| {
        // Tight inside the name; the actions on the right keep the usual spacing.
        let spacing = std::mem::replace(&mut ui.spacing_mut().item_spacing.x, 0.0);
        if cols.swatch {
            let (rect, _) = ui.allocate_exact_size(vec2(label_x - icon_x, repo_swatch::SIZE), Sense::hover());
            if let Some(c) = group.hue() {
                let slot = Rect::from_min_size(rect.min, vec2(menu_columns::ICON_W, rect.height()));
                repo_swatch::paint_slot(ui.painter(), slot, color::repo_hue(c));
            }
        }
        let name = egui::Label::new(RichText::new(&group.name).color(color::weak())).selectable(false).sense(Sense::click());
        ui.add(name).row_menu(|ui| group_menu(ui, st, g, &mut menu_msgs));
        ui.add_space(8.0);
        ui.label(RichText::new(paths.len().to_string()).size(type_scale::SMALL).color(color::faint()));
        ui.spacing_mut().item_spacing.x = spacing;
    };
    let mut open_all = false;
    let mut more = None;
    let right = |ui: &mut egui::Ui| {
        more = Some(widgets::small_icon(ui, Icon::More, t.tr("repo_group.actions.tip"), color::weak()));
        open_all = unopened && small_link(ui, t.tr("repo_group.menu.open_all"));
    };
    header_row(ui, if cols.swatch { icon_x } else { label_x }, left, right);
    msgs.append(&mut menu_msgs);
    if let Some(more) = more {
        menu::below(ui, &more, |ui| group_menu(ui, st, g, msgs));
    }
    if open_all {
        msgs.push(Msg::Recent(RecentMsg::OpenAll(paths)));
    }
}

/// The group menu of group `g`.
fn group_menu(ui: &mut egui::Ui, st: &AppState, g: usize, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let count = st.settings.repo_groups.len();
    let group = &st.settings.repo_groups[g];
    menu_item(msgs, ui, t.tr("repo_group.menu.open_all"), Msg::Recent(RecentMsg::OpenAll(members(&st.settings, g))));
    menu_item(msgs, ui, t.tr("repo_group.menu.add_repos"), Msg::Groups(GroupMsg::AddDialog(g)));
    menu_item(msgs, ui, t.tr("repo_group.menu.rename"), Msg::Groups(GroupMsg::RenameDialog(g)));
    let set = |colour| Msg::Groups(GroupMsg::SetColour { group: g, colour });
    super::repo_colour_menu::hues(ui, t, group.hue(), t.tr("repo_colour.none.tip"), set, msgs);
    let at_default = colours_at_default(&st.settings).then(|| t.tr("repo_group.menu.reset_colours.blocked"));
    menu_item_if(msgs, ui, t.tr("repo_group.menu.reset_colours"), at_default, Msg::Groups(GroupMsg::ResetColours));
    separator(ui);
    let first = (g == 0).then(|| t.tr("repo_group.menu.move_up.blocked"));
    let last = (g + 1 >= count).then(|| t.tr("repo_group.menu.move_down.blocked"));
    let up = Msg::Groups(GroupMsg::MoveGroup { from: g, to: g.saturating_sub(1) });
    menu_item_if(msgs, ui, t.tr("repo_group.menu.move_up"), first, up);
    menu_item_if(msgs, ui, t.tr("repo_group.menu.move_down"), last, Msg::Groups(GroupMsg::MoveGroup { from: g, to: g + 1 }));
    separator(ui);
    menu_danger(msgs, ui, t.tr("repo_group.menu.remove"), None, Msg::Groups(GroupMsg::Remove(g)));
}

/// An empty section's placeholder, a row's height: an outlined dashed box with `text` where the
/// rows' names start (`cols`) and, with `link`, that small link at its right, on the column of
/// the headers' Open all. Returns its rect, which a drag can drop onto, and whether the link was
/// clicked.
pub(super) fn placeholder(ui: &mut egui::Ui, text: &str, link: Option<&str>, cols: Columns) -> (Rect, bool) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), repo_row::ROW_H), Sense::hover());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
    let r = rect.shrink(0.5);
    let p = ui.painter();
    let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    p.extend(Shape::dashed_line(&corners, Stroke::new(1.0_f32, color::border()), DASH, DASH_GAP));
    let (_, label_x) = repo_row::lead(ui, cols);
    let g = p.layout_no_wrap(text.to_owned(), type_scale::font(type_scale::SMALL), color::weak());
    p.galley(pos2(rect.left() + label_x, rect.center().y - g.size().y / 2.0), g, color::weak());
    let clicked = link.is_some_and(|link| {
        let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(Layout::right_to_left(Align::Center)));
        // Past the ⋯ slot, as `header_row` lays out a group's header.
        ui.add_space(repo_row::END_PAD);
        let h = ControlSize::Small.metrics().h;
        ui.allocate_exact_size(vec2(h, h), Sense::hover());
        small_link(&mut ui, link)
    });
    (rect, clicked)
}

/// The bar down a group's rows, from `top` to `bottom`, left of rows starting at `left`, in the
/// group's colour (the border's without one).
pub(super) fn bar(ui: &egui::Ui, left: f32, top: f32, bottom: f32, hue: Option<Color32>) {
    let x = left - BAR_GAP;
    let r = Rect::from_min_max(pos2(x - BAR_W, top), pos2(x, bottom));
    ui.painter().rect_filled(r, CornerRadius::same(radius::XS), hue.unwrap_or_else(color::border));
}

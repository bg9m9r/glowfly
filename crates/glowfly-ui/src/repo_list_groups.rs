//! The welcome screen's repository group headers (`repo_list.rs` draws the rows): the group's
//! swatch and name, and at the right ⋯ (Group actions), Open all while one of its repositories
//! isn't in a tab, and on the first section's header the New group link. The group menu (from
//! ⋯, a right-click or Shift+F10 on the name): Open all, Rename…, Color, Move up and Move down,
//! and Remove group in the danger style. A section without rows shows a weak line in a row's
//! height, which dragging drops onto.

use super::common::{menu_danger, menu_item, menu_item_if};
use super::repo_list::{header_text, small_link};
use super::repo_swatch;
use super::theme::{color, type_scale};
use super::widgets::{self, Icon};
use crate::msg::{GroupMsg, Msg, RecentMsg};
use crate::state::AppState;
use crate::state::repo_groups::members;
use crate::state::repo_lists::{is_missing, is_open};
use crate::ui::widgets::menu::{self, separator};
use crate::ui::widgets::row_menu::RowMenuExt;
use eframe::egui::{self, Align, Layout, Rect, RichText, Sense};
use glowfly_core::i18n::Lang;

/// The header of group `g`; `first` when it is the first section (it carries New group).
pub(super) fn header(ui: &mut egui::Ui, st: &AppState, g: usize, first: bool, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let group = &st.settings.repo_groups[g];
    let paths = members(&st.settings, g);
    let unopened = paths.iter().any(|p| !is_open(st, p) && !is_missing(st, p));
    ui.horizontal(|ui| {
        menu::inset(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(c) = group.hue() {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(repo_swatch::SIZE, repo_swatch::SIZE), Sense::hover());
                    repo_swatch::paint(ui.painter(), rect.left_center(), repo_swatch::SIZE, color::repo_hue(c));
                }
                let name = egui::Label::new(header_text(&group.name)).selectable(false).sense(Sense::click());
                ui.add(name).row_menu(|ui| group_menu(ui, st, g, msgs));
            });
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let more = widgets::small_icon(ui, Icon::More, t.tr("repo_group.actions.tip"), color::weak());
            menu::below(ui, &more, |ui| group_menu(ui, st, g, msgs));
            if unopened && small_link(ui, t.tr("repo_group.menu.open_all")) {
                msgs.push(Msg::Recent(RecentMsg::OpenAll(paths.clone())));
            }
            if first {
                new_group_link(ui, t, msgs);
            }
        });
    });
}

/// The New group link of the first section's header.
pub(super) fn new_group_link(ui: &mut egui::Ui, t: Lang, msgs: &mut Vec<Msg>) {
    if small_link(ui, t.tr("repos.groups.new.link")) {
        msgs.push(Msg::Groups(GroupMsg::NewDialog { repo: None }));
    }
}

/// The group menu of group `g`.
fn group_menu(ui: &mut egui::Ui, st: &AppState, g: usize, msgs: &mut Vec<Msg>) {
    let t = st.settings.lang();
    let count = st.settings.repo_groups.len();
    let group = &st.settings.repo_groups[g];
    menu_item(msgs, ui, t.tr("repo_group.menu.open_all"), Msg::Recent(RecentMsg::OpenAll(members(&st.settings, g))));
    menu_item(msgs, ui, t.tr("repo_group.menu.rename"), Msg::Groups(GroupMsg::RenameDialog(g)));
    let set = |colour| Msg::Groups(GroupMsg::SetColour { group: g, colour });
    super::repo_colour_menu::hues(ui, t, group.hue(), t.tr("repo_colour.none.tip"), set, msgs);
    separator(ui);
    let first = (g == 0).then(|| t.tr("repo_group.menu.move_up.blocked"));
    let last = (g + 1 >= count).then(|| t.tr("repo_group.menu.move_down.blocked"));
    let up = Msg::Groups(GroupMsg::MoveGroup { from: g, to: g.saturating_sub(1) });
    menu_item_if(msgs, ui, t.tr("repo_group.menu.move_up"), first, up);
    menu_item_if(msgs, ui, t.tr("repo_group.menu.move_down"), last, Msg::Groups(GroupMsg::MoveGroup { from: g, to: g + 1 }));
    separator(ui);
    menu_danger(msgs, ui, t.tr("repo_group.menu.remove"), None, Msg::Groups(GroupMsg::Remove(g)));
}

/// A weak line where a section has no rows (`text` says what to do); returns its rect, which
/// a drag can drop onto.
pub(super) fn placeholder(ui: &mut egui::Ui, text: &str) -> Rect {
    let h = ui.spacing().interact_size.y;
    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), h), Layout::left_to_right(Align::Center), |ui| {
        menu::inset(ui, |ui| ui.add(egui::Label::new(RichText::new(text).size(type_scale::SMALL).color(color::weak())).truncate()));
    })
    .response
    .rect
}

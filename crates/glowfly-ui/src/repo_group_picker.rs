//! A repository group's Add repositories dialog (`Modal::AddToGroup`, from the group's ⋯ and its
//! empty placeholder): "Add to {group}" with the group's swatch, then one checklist in two
//! sections, Favorites (those not in the group; one in another group says so at the right, as
//! ticking it moves it) and Recent (starred when added). A row is the box, the name and its
//! parent folder, and a click anywhere on it ticks it. Past `FILTER_FROM` rows a filter field
//! leads, focused; ticks are kept by path, so a hidden row stays ticked and counted on
//! "Add {count}". With nothing to offer, a line says so and the footer has only Close.

use super::dialog_frame::{Cx, Dialog, Footer};
use super::repo_row::ROW_H;
use super::repo_swatch;
use super::theme::{color, controls, type_scale};
use super::widgets::{TextField, buttons, row_fill, text_field};
use crate::msg::{GroupMsg, Msg};
use crate::state::repo_group_picker::{AddPicker, PickRow};
use eframe::egui::{self, Align, Layout, RichText, Sense, UiBuilder, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::native_path::shown;
use glowfly_core::settings::RepoGroup;

/// The list's most height before it scrolls (ten rows).
const LIST_H: f32 = 320.0;
/// Between the box and the name, and the name and its folder.
const BOX_GAP: f32 = 8.0;
const NAME_GAP: f32 = 12.0;
/// Above a section's heading (but the first).
const SECTION_GAP: f32 = 8.0;

/// Draws the dialog; returns true when Cancel, Close or Esc closes it. `update` closes it after
/// an Add.
pub fn show(ui: &mut egui::Ui, cx: Cx, form: &mut AddPicker, groups: &[RepoGroup], msgs: &mut Vec<Msg>) -> bool {
    let t = cx.t;
    let Some(group) = groups.get(form.group) else { return true };
    let mut d = Dialog::new(cx, t.trf("repo_group.add.title", &[("name", &group.name)]));
    if let Some(c) = group.hue() {
        d = d.swatch(color::repo_hue(c));
    }
    let mut d = d.begin(ui);
    if form.rows.is_empty() {
        ui.add(egui::Label::new(RichText::new(t.tr("repo_group.add.nothing")).color(color::weak())).wrap());
        return d.finish(ui, Footer::new().close()).dismiss;
    }
    let mut entered = false;
    if form.filtered() {
        let r = ui.add(TextField::new(&mut form.filter).hint(t.tr("finder.filter.hint")));
        entered = text_field::entered(ui, &r);
        if cx.keyboard && ui.memory(|m| m.focused().is_none()) {
            r.request_focus();
        }
        ui.add_space(6.0);
    }
    let toggled = d.body(ui, |ui| checklist(ui, t, form, groups));
    if let Some(r) = toggled {
        form.toggle(&r);
    }
    ui.add_space(4.0);
    let picked = form.picked();
    let label = match picked.len() {
        0 => t.tr("repo_group.add.button_none").to_owned(),
        n => t.trf("repo_group.add.button", &[("count", &n)]),
    };
    let none = picked.is_empty().then(|| t.tr("repo_group.add.none_ticked"));
    let add = buttons::primary(label).enabled(!picked.is_empty()).why(none);
    let out = d.finish(ui, Footer::new().cancel().primary(add).submit(entered && !picked.is_empty()));
    if out.primary {
        msgs.push(Msg::Groups(GroupMsg::AddRepos { group: form.group, paths: picked }));
    }
    out.dismiss
}

/// The two sections in a scrolling list; returns the row clicked this frame.
fn checklist(ui: &mut egui::Ui, t: Lang, form: &AddPicker, groups: &[RepoGroup]) -> Option<PickRow> {
    let rows: Vec<&PickRow> = form.shown().collect();
    let mut clicked = None;
    egui::ScrollArea::vertical().id_salt("repo-group-picker").max_height(LIST_H).auto_shrink([false, true]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        if rows.is_empty() {
            ui.label(RichText::new(t.tr("finder.no_match")).color(color::weak()));
        }
        let (favs, recent): (Vec<&PickRow>, Vec<&PickRow>) = rows.iter().partition(|r| !r.recent);
        for (i, (title, list)) in [(t.tr("repos.favorites.header"), favs), (t.tr("repos.recent.header"), recent)].into_iter().enumerate() {
            if list.is_empty() {
                continue;
            }
            if i > 0 {
                ui.add_space(SECTION_GAP);
            }
            heading(ui, title, (i > 0).then(|| t.tr("repo_group.add.recent_note")));
            for r in list {
                if row(ui, t, r, form.is_ticked(r), groups) {
                    clicked = Some(r.clone());
                }
            }
        }
    });
    clicked
}

/// A section's weak heading, with a weak note at its right.
fn heading(ui: &mut egui::Ui, title: &str, note: Option<&str>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(type_scale::SMALL).color(color::weak()));
        if let Some(n) = note {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(n).size(type_scale::SMALL).color(color::weak()));
            });
        }
    });
    ui.add_space(2.0);
}

/// One row: box, name, parent folder and, for a favorite in another group, "In {group}" with
/// its swatch. Returns whether it was clicked (on the box or anywhere else on it).
fn row(ui: &mut egui::Ui, t: Lang, r: &PickRow, ticked: bool, groups: &[RepoGroup]) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    row_fill::paint(ui.painter(), rect, false, resp.hovered());
    let full = shown(&r.path);
    let name = r.path.file_name().map_or_else(|| full.clone(), |n| n.to_string_lossy().into_owned());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, ticked, &name));
    let folder = r.path.parent().map(shown).unwrap_or_default();
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect.shrink2(vec2(4.0, 0.0))).layout(Layout::left_to_right(Align::Center)));
    let ui = &mut child;
    ui.spacing_mut().item_spacing.x = 0.0;
    let mut on = ticked;
    let boxed = ui.add(controls::Check::without_text(&mut on)).changed();
    ui.add_space(BOX_GAP);
    ui.add(egui::Label::new(RichText::new(&name).size(type_scale::BODY).color(color::text())).selectable(false));
    ui.add_space(NAME_GAP);
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if let Some(g) = r.other_group.and_then(|g| groups.get(g)) {
            ui.label(RichText::new(t.trf("repo_group.add.in_group", &[("name", &g.name)])).size(type_scale::SMALL).color(color::weak()));
            if let Some(c) = g.hue() {
                ui.add_space(6.0);
                let (slot, _) = ui.allocate_exact_size(vec2(repo_swatch::SIZE, repo_swatch::SIZE), Sense::hover());
                repo_swatch::paint_slot(ui.painter(), slot, color::repo_hue(c));
            }
            ui.add_space(NAME_GAP);
        }
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            let label = egui::Label::new(RichText::new(folder).size(type_scale::SMALL).color(color::weak())).truncate().selectable(false);
            ui.add(label);
        });
    });
    boxed || resp.on_hover_text(full).clicked()
}

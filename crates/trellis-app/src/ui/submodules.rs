//! The SUBMODULES section of the left panel: one row per submodule with a painted status
//! marker, a status word and an explanatory tooltip; double-click opens it, and the row menu
//! opens, initializes, updates or restores it, copies its URL, edits or removes it. The header's
//! menu adds one, initializes or updates them all, and chooses whether this repository updates
//! them after checkouts and pulls; its + adds one too. The section always shows: without
//! submodules, an empty state line offers Add submodule….

use super::common::menu_item_if;
use super::theme::color;
use super::widgets;
use super::widgets::section_header::{self, Header, HeaderAdd};
use crate::msg::Msg;
use crate::state::section_actions::SectionAction;
use crate::state::submodules::batch::{init_all_blocked, update_all_blocked};
use crate::state::submodules::remove::remove_blocked;
use crate::state::submodules::{
    SectionCx, copy_url_blocked, copy_url_text, edit_blocked, open_blocked, row_word, tooltip, update_blocked, update_label,
};
use crate::submodule::{Batch, SubmoduleMsg};
use crate::ui::theme::controls;
use eframe::egui::{self, Painter, Pos2, Stroke, pos2, vec2};
use trellis_core::i18n::Lang;
use trellis_core::repo::{Snapshot, SubmoduleInfo, SubmoduleState};
use trellis_core::settings::SubmoduleUpdates;

/// The row filter, whether a filter is typed, and the section's add action.
type Filtered<'a> = (&'a dyn Fn(&str) -> bool, bool, &'a SectionAction);

pub(super) fn section(
    ui: &mut egui::Ui,
    t: Lang,
    snap: &Snapshot,
    cx: &SectionCx,
    open: &mut bool,
    (shown, filtering, add): Filtered,
    msgs: &mut Vec<Msg>,
) {
    let items = |ui: &mut egui::Ui, msgs: &mut Vec<Msg>| header_menu(ui, snap, cx, add, msgs);
    let title = t.tr("sidebar.submodules.header");
    let h = Header { key: "submodules", title, count: Some(snap.submodules.len()), add: Some(HeaderAdd::send(add)), menu: Some(&items), t };
    section_header::show(ui, h, open, msgs);
    if !*open {
        return;
    }
    if snap.submodules.is_empty() && !filtering {
        section_header::empty_row(ui, t.tr("sidebar.submodules.empty"), add, msgs);
    }
    for sm in snap.submodules.iter().filter(|s| shown(&s.path) || shown(&s.name)) {
        let col = match sm.state {
            SubmoduleState::Uninitialized => color::weak(),
            SubmoduleState::Missing => color::removed(),
            _ => color::warn(),
        };
        let word = row_word(sm);
        let right = word.as_deref().map(|w| [widgets::Part::Text(w)]);
        let (r, _, cut) =
            widgets::list_row_cut(ui, widgets::ROW_INDENT, &sm.path, right.as_ref().map(|p| (&p[..], col)), 0.0, false, false);
        marker(ui.painter(), pos2(r.rect.left() + widgets::GUTTER_X, r.rect.center().y), sm);
        let r = r.on_hover_text(widgets::truncate::full_tip(cut, &sm.path, Some(tooltip(sm))).unwrap_or_default());
        if r.double_clicked() && open_blocked(sm).is_none() {
            msgs.push(Msg::Submodule(SubmoduleMsg::Open(sm.path.clone())));
        }
        r.context_menu(|ui| menu(ui, sm, snap, cx, msgs));
    }
}

/// Add, the batch items (left out while there are none), and the update choice.
fn header_menu(ui: &mut egui::Ui, snap: &Snapshot, cx: &SectionCx, add: &SectionAction, msgs: &mut Vec<Msg>) {
    let batch = |batch| Msg::Submodule(SubmoduleMsg::Batch { batch, confirmed: false });
    section_header::action_item(ui, msgs, add);
    if !snap.submodules.is_empty() {
        menu_item_if(msgs, ui, "Initialize all submodules", init_all_blocked(&snap.submodules), batch(Batch::InitAll));
        menu_item_if(msgs, ui, "Update all submodules", update_all_blocked(&snap.submodules), batch(Batch::UpdateAll));
    }
    ui.separator();
    ui.menu_button("Update after checkout and pull", |ui| {
        let follow = if cx.global { "Follow preferences (on)" } else { "Follow preferences (off)" };
        for (choice, label) in [
            (SubmoduleUpdates::FollowGlobal, follow),
            (SubmoduleUpdates::Always, "Always in this repository"),
            (SubmoduleUpdates::Never, "Never in this repository"),
        ] {
            if controls::radio(ui, cx.updates == choice, label).clicked() {
                msgs.push(Msg::Submodule(SubmoduleMsg::SetUpdates(choice)));
                ui.close_menu();
            }
        }
    });
}

fn menu(ui: &mut egui::Ui, sm: &SubmoduleInfo, snap: &Snapshot, cx: &SectionCx, msgs: &mut Vec<Msg>) {
    let path = sm.path.clone();
    menu_item_if(msgs, ui, "Open in new tab", open_blocked(sm), Msg::Submodule(SubmoduleMsg::Open(path.clone())));
    let update = Msg::Submodule(SubmoduleMsg::Update { path: path.clone(), confirmed: false });
    menu_item_if(msgs, ui, update_label(sm), update_blocked(sm), update);
    menu_item_if(msgs, ui, "Copy URL", copy_url_blocked(sm), Msg::Copy(copy_url_text(sm)));
    let edit = edit_blocked(sm, cx.edit_git.as_deref());
    menu_item_if(msgs, ui, "Edit submodule…", edit.as_deref(), Msg::Submodule(SubmoduleMsg::EditDialog(path)));
    ui.separator();
    let remove = Msg::Submodule(SubmoduleMsg::RemoveDialog(sm.path.clone()));
    menu_item_if(msgs, ui, "Remove submodule…", remove_blocked(sm, snap), remove);
}

/// The row's status marker, painted (the bundled fonts lack suitable glyphs). An added
/// submodule gets a small plus at the top right.
fn marker(p: &Painter, c: Pos2, sm: &SubmoduleInfo) {
    let stroke = Stroke::new(1.5_f32, color::warn());
    match sm.state {
        SubmoduleState::InSync => {
            p.circle_filled(c, 4.0, color::added());
        }
        SubmoduleState::OutOfSync => {
            p.circle_filled(c, 4.0, color::warn());
        }
        SubmoduleState::Dirty => {
            p.circle_stroke(c, 4.0, stroke);
        }
        SubmoduleState::DirtyOutOfSync => {
            p.circle_filled(c, 4.0, color::warn());
            p.circle_stroke(c, 6.0, stroke);
        }
        SubmoduleState::Uninitialized => widgets::dashed_circle(p, c, 4.0, Stroke::new(1.5_f32, color::weak())),
        SubmoduleState::Missing => {
            let red = Stroke::new(1.5_f32, color::removed());
            widgets::dashed_circle(p, c, 4.0, red);
            p.line_segment([c + vec2(-3.5, 3.5), c + vec2(3.5, -3.5)], red);
        }
    }
    if sm.added {
        let plus = c + vec2(5.0, -5.0);
        let green = Stroke::new(1.5_f32, color::added());
        p.line_segment([plus - vec2(2.5, 0.0), plus + vec2(2.5, 0.0)], green);
        p.line_segment([plus - vec2(0.0, 2.5), plus + vec2(0.0, 2.5)], green);
    }
}

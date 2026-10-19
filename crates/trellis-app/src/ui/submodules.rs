//! The SUBMODULES section of the left panel: one row per submodule with a painted status
//! marker, a status word and an explanatory tooltip; double-click opens it, and the row menu
//! opens, initializes, updates or restores it, copies its URL, edits or removes it. The header's
//! menu adds one, initializes or updates them all, and chooses whether this repository updates
//! them after checkouts and pulls (the palette's Add submodule… adds one too, which is how the
//! first one is added: the section only shows when there are submodules).

use super::common::{menu_item, menu_item_if};
use super::theme::color;
use super::widgets;
use crate::msg::Msg;
use crate::state::submodules::batch::{init_all_blocked, update_all_blocked};
use crate::state::submodules::remove::remove_blocked;
use crate::state::submodules::{
    SectionCx, copy_url_blocked, copy_url_text, edit_blocked, open_blocked, row_word, tooltip, update_blocked, update_label,
};
use crate::submodule::{Batch, SubmoduleMsg};
use crate::ui::theme::controls;
use eframe::egui::{self, Painter, Pos2, Stroke, pos2, vec2};
use trellis_core::repo::{Snapshot, SubmoduleInfo, SubmoduleState};
use trellis_core::settings::SubmoduleUpdates;

pub(super) fn section(
    ui: &mut egui::Ui,
    snap: &Snapshot,
    cx: &SectionCx,
    open: &mut bool,
    shown: &dyn Fn(&str) -> bool,
    msgs: &mut Vec<Msg>,
) {
    let h = widgets::section_header(ui, "SUBMODULES", Some(snap.submodules.len()), open);
    h.context_menu(|ui| header_menu(ui, snap, cx, msgs));
    if !*open {
        return;
    }
    for sm in snap.submodules.iter().filter(|s| shown(&s.path) || shown(&s.name)) {
        let col = match sm.state {
            SubmoduleState::Uninitialized => color::weak(),
            SubmoduleState::Missing => color::red(),
            _ => color::amber(),
        };
        let word = row_word(sm);
        let r = widgets::list_row(ui, 26.0, &sm.path, word.as_deref().map(|w| (w, col)), false, false);
        marker(ui.painter(), pos2(r.rect.left() + 16.0, r.rect.center().y), sm);
        let r = r.on_hover_text(tooltip(sm));
        if r.double_clicked() && open_blocked(sm).is_none() {
            msgs.push(Msg::Submodule(SubmoduleMsg::Open(sm.path.clone())));
        }
        r.context_menu(|ui| menu(ui, sm, snap, cx, msgs));
    }
}

fn header_menu(ui: &mut egui::Ui, snap: &Snapshot, cx: &SectionCx, msgs: &mut Vec<Msg>) {
    let batch = |batch| Msg::Submodule(SubmoduleMsg::Batch { batch, confirmed: false });
    menu_item(msgs, ui, "Add submodule…", Msg::Submodule(SubmoduleMsg::AddDialog));
    menu_item_if(msgs, ui, "Initialize all submodules", init_all_blocked(&snap.submodules), batch(Batch::InitAll));
    menu_item_if(msgs, ui, "Update all submodules", update_all_blocked(&snap.submodules), batch(Batch::UpdateAll));
    ui.separator();
    ui.menu_button("Update after checkout and pull", |ui| {
        let follow = if cx.global { "Follow Preferences (on)" } else { "Follow Preferences (off)" };
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
    let stroke = Stroke::new(1.5_f32, color::amber());
    match sm.state {
        SubmoduleState::InSync => {
            p.circle_filled(c, 4.0, color::green());
        }
        SubmoduleState::OutOfSync => {
            p.circle_filled(c, 4.0, color::amber());
        }
        SubmoduleState::Dirty => {
            p.circle_stroke(c, 4.0, stroke);
        }
        SubmoduleState::DirtyOutOfSync => {
            p.circle_filled(c, 4.0, color::amber());
            p.circle_stroke(c, 6.0, stroke);
        }
        SubmoduleState::Uninitialized => widgets::dashed_circle(p, c, 4.0, Stroke::new(1.5_f32, color::weak())),
        SubmoduleState::Missing => {
            let red = Stroke::new(1.5_f32, color::red());
            widgets::dashed_circle(p, c, 4.0, red);
            p.line_segment([c + vec2(-3.5, 3.5), c + vec2(3.5, -3.5)], red);
        }
    }
    if sm.added {
        let plus = c + vec2(5.0, -5.0);
        let green = Stroke::new(1.5_f32, color::green());
        p.line_segment([plus - vec2(2.5, 0.0), plus + vec2(2.5, 0.0)], green);
        p.line_segment([plus - vec2(0.0, 2.5), plus + vec2(0.0, 2.5)], green);
    }
}

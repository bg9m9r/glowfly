//! The SUBMODULES section of the left panel: one row per submodule with a painted status
//! marker, a status word and an explanatory tooltip; double-click opens it, and the row menu
//! opens, initializes or updates it, copies its URL or removes it. The header's menu adds one
//! (the palette's Add submodule… does too, which is how the first one is added: the section only
//! shows when there are submodules).

use super::common::{menu_item, menu_item_if};
use super::theme::color;
use super::widgets;
use crate::msg::Msg;
use crate::state::submodules::remove::remove_blocked;
use crate::state::submodules::{copy_url_blocked, open_blocked, state_word, tooltip, update_blocked, update_label};
use crate::submodule::SubmoduleMsg;
use eframe::egui::{self, Painter, Pos2, Stroke, pos2};
use trellis_core::repo::{Snapshot, SubmoduleInfo, SubmoduleState};

pub(super) fn section(ui: &mut egui::Ui, snap: &Snapshot, open: &mut bool, shown: &dyn Fn(&str) -> bool, msgs: &mut Vec<Msg>) {
    let h = widgets::section_header(ui, "SUBMODULES", Some(snap.submodules.len()), open);
    h.context_menu(|ui| menu_item(msgs, ui, "Add submodule…", Msg::Submodule(SubmoduleMsg::AddDialog)));
    if !*open {
        return;
    }
    for sm in snap.submodules.iter().filter(|s| shown(&s.path) || shown(&s.name)) {
        let col = if sm.state == SubmoduleState::Uninitialized { color::weak() } else { color::amber() };
        let word = state_word(sm.state);
        let r = widgets::list_row(ui, 26.0, &sm.path, word.map(|w| (w, col)), false, false);
        marker(ui.painter(), pos2(r.rect.left() + 16.0, r.rect.center().y), sm.state);
        let r = r.on_hover_text(tooltip(sm));
        if r.double_clicked() && open_blocked(sm).is_none() {
            msgs.push(Msg::Submodule(SubmoduleMsg::Open(sm.path.clone())));
        }
        r.context_menu(|ui| menu(ui, sm, snap, msgs));
    }
}

fn menu(ui: &mut egui::Ui, sm: &SubmoduleInfo, snap: &Snapshot, msgs: &mut Vec<Msg>) {
    let path = sm.path.clone();
    menu_item_if(msgs, ui, "Open in new tab", open_blocked(sm), Msg::Submodule(SubmoduleMsg::Open(path.clone())));
    menu_item_if(msgs, ui, update_label(sm), update_blocked(sm), Msg::Submodule(SubmoduleMsg::Update { path, confirmed: false }));
    menu_item_if(msgs, ui, "Copy URL", copy_url_blocked(sm), Msg::Copy(sm.url.clone().unwrap_or_default()));
    ui.separator();
    let remove = Msg::Submodule(SubmoduleMsg::RemoveDialog(sm.path.clone()));
    menu_item_if(msgs, ui, "Remove submodule…", remove_blocked(sm, snap), remove);
}

/// The row's status marker, painted (the bundled fonts lack suitable glyphs).
fn marker(p: &Painter, c: Pos2, state: SubmoduleState) {
    let stroke = Stroke::new(1.5_f32, color::amber());
    match state {
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
    }
}

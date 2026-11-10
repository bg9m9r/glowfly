//! The SUBMODULES section of the left panel: one row per submodule with a painted status
//! marker (with its own short tooltip), a status word and an explanatory tooltip; double-click opens it, and the row menu
//! opens, initializes, updates or restores it, records its checkout, fetches or pulls inside it,
//! copies its URL, edits it, discards the changes inside it or removes it. The header's
//! menu adds one, initializes or updates them all, and chooses whether this repository updates
//! them after checkouts and pulls; its + adds one too. The section always shows: without
//! submodules, once expanded, a sentence on what they are for and Add submodule….

use super::common::{menu_danger, menu_item_if};
use super::theme::color;
use super::widgets;
use super::widgets::empty_state::{self, Action};
use super::widgets::reason::{self, ItemState};
use super::widgets::section_header::{self, Header, HeaderAdd};
use crate::widgets::menu::{self, separator, submenu};
use crate::widgets::row_menu::RowMenuExt;
use eframe::egui::{self, Painter, Pos2, Rect, Stroke, pos2, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{Snapshot, SubmoduleInfo, SubmoduleState};
use glowfly_core::settings::SubmoduleUpdates;
use glowfly_model::msg::Msg;
use glowfly_model::state::section_actions::SectionAction;
use glowfly_model::state::submodules::batch::{init_all_blocked, update_all_blocked};
use glowfly_model::state::submodules::inside;
use glowfly_model::state::submodules::remove::remove_blocked;
use glowfly_model::state::submodules::{
    SectionCx, copy_url_blocked, copy_url_text, dot_tip, edit_blocked, open_blocked, row_word, tooltip, update_blocked, update_label,
};
use glowfly_model::submodule::{Batch, SubmoduleMsg};

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
    let h = Header {
        key: "submodules",
        title,
        count: Some(snap.submodules.len()),
        folds: true,
        add: Some(HeaderAdd::send(add)),
        menu: Some(&items),
        t,
    };
    section_header::show(ui, h, open, msgs);
    if !*open {
        return;
    }
    if snap.submodules.is_empty() && !filtering {
        empty_state::show(ui, t.tr("sidebar.empty.submodules"), vec![Action::of(add)], msgs);
    }
    for sm in snap.submodules.iter().filter(|s| shown(&s.path) || shown(&s.name)) {
        let col = match sm.state {
            SubmoduleState::InSync if !sm.added => color::weak(),
            SubmoduleState::Uninitialized => color::weak(),
            SubmoduleState::Missing => color::removed(),
            _ => color::warn(),
        };
        let word = row_word(t, sm);
        let right = word.as_deref().map(|w| [widgets::Part::Text(w)]);
        let (r, _, cut) =
            widgets::list_row_cut(ui, widgets::ROW_INDENT, &sm.path, right.as_ref().map(|p| (&p[..], col)), 0.0, false, false);
        let centre = pos2(r.rect.left() + widgets::GUTTER_X, r.rect.center().y);
        marker(ui.painter(), centre, sm);
        // Over the dot, what it means; over the rest of the row, the long explanation. Chosen
        // from the row's own hover (a separate hover area over it would fight its clicks).
        let dot = Rect::from_center_size(centre, vec2(16.0, 16.0));
        let tip = if r.hover_pos().is_some_and(|p| dot.contains(p)) {
            dot_tip(t, sm)
        } else {
            widgets::truncate::full_tip(cut, &sm.path, Some(tooltip(sm))).unwrap_or_default()
        };
        let r = r.on_hover_text(tip);
        if r.double_clicked() && open_blocked(sm).is_none() {
            msgs.push(Msg::Submodule(SubmoduleMsg::Open(sm.path.clone())));
        }
        r.row_menu(|ui| menu(ui, t, sm, snap, cx, msgs));
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
    separator(ui);
    submenu(ui, "Update after checkout and pull", |ui| {
        let follow = if cx.global { "Follow preferences (on)" } else { "Follow preferences (off)" };
        for (choice, label) in [
            (SubmoduleUpdates::FollowGlobal, follow),
            (SubmoduleUpdates::Always, "Always in this repository"),
            (SubmoduleUpdates::Never, "Never in this repository"),
        ] {
            if menu::item(label).option(cx.updates == choice).show(ui).clicked() {
                msgs.push(Msg::Submodule(SubmoduleMsg::SetUpdates(choice)));
                ui.close_menu();
            }
        }
    });
}

fn menu(ui: &mut egui::Ui, t: Lang, sm: &SubmoduleInfo, snap: &Snapshot, cx: &SectionCx, msgs: &mut Vec<Msg>) {
    let path = sm.path.clone();
    let m = |s: SubmoduleMsg| Msg::Submodule(s);
    menu_item_if(msgs, ui, "Open in new tab", open_blocked(sm), m(SubmoduleMsg::Open(path.clone())));
    let update = m(SubmoduleMsg::Update { path: path.clone(), confirmed: false });
    menu_item_if(msgs, ui, update_label(sm), update_blocked(sm), update);
    // Its tip says what is recorded, and what isn't.
    let blocked = inside::record_blocked(t, sm, snap);
    let record = reason::menu_button(ui, t.tr("sidebar.submodules.menu.record"), "", ItemState::blocked_by(blocked));
    let record = if blocked.is_none() { record.on_hover_text(t.tr("sidebar.submodules.record.tip")) } else { record };
    if record.clicked() {
        msgs.push(m(SubmoduleMsg::Record(path.clone())));
        ui.close_menu();
    }
    separator(ui);
    menu_item_if(msgs, ui, t.tr("sidebar.submodules.menu.fetch"), inside::fetch_blocked(t, sm), m(SubmoduleMsg::Fetch(path.clone())));
    menu_item_if(msgs, ui, t.tr("sidebar.submodules.menu.pull"), inside::pull_blocked(t, sm), m(SubmoduleMsg::Pull(path.clone())));
    separator(ui);
    menu_item_if(msgs, ui, "Copy URL", copy_url_blocked(sm), Msg::Copy(copy_url_text(sm)));
    let edit = edit_blocked(sm, cx.edit_git.as_deref());
    menu_item_if(msgs, ui, "Edit submodule…", edit.as_deref(), m(SubmoduleMsg::EditDialog(path.clone())));
    separator(ui);
    let discard = m(SubmoduleMsg::DiscardInside { path, confirmed: false });
    menu_danger(msgs, ui, t.tr("sidebar.submodules.menu.discard_inside"), inside::discard_blocked(t, sm), discard);
    let remove = m(SubmoduleMsg::RemoveDialog(sm.path.clone()));
    menu_danger(msgs, ui, "Remove submodule…", remove_blocked(sm, snap), remove);
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

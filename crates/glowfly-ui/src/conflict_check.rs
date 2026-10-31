//! Predicted merge conflicts: the toolbar chip (the coral needs-you pill: the warning mark and
//! the number of conflicting files in its ink) with its popover, and the report dialog for checks the branch menus ask
//! for. Texts come from state/conflict_check.rs; the buttons send the same Merge and Rebase
//! jobs as the branch menus.

use super::dialog_frame::{Cx, Dialog, Footer, Size};
use super::theme::color;
use super::widgets::buttons;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::conflict_check::{self as cc, ConflictReport};
use crate::ui::theme::tokens::radius;
use crate::ui::theme::type_scale;
use crate::ui::widgets::menu::popover_below;
use eframe::egui::{self, Id, RichText, Sense, pos2, vec2};
use glowfly_core::branch_ops::{merge_cmd, rebase_cmd};
use glowfly_core::conflict_check::Prediction;
use glowfly_core::i18n::Lang;
use glowfly_model::conflict_check::ConflictCheckMsg;

/// The chip's height; the toolbar row is 48px.
const HEIGHT: f32 = 24.0;
/// The chip's click target and its popover (one of each per window).
const CHIP_ID: &str = "conflict-chip";
const POPOVER_ID: &str = "conflict-popover";

/// The check the chip shows and its conflicting paths, while it is drawn.
pub fn shown(st: &AppState) -> Option<(&ConflictReport, &Vec<String>)> {
    if !st.settings.conflict_check {
        return None;
    }
    let current = cc::current_request(st.snap.as_ref()?);
    let report = st.session.reads.conflict_checks.shown(current.as_ref())?;
    let Ok(Prediction::Conflicts(paths)) = &report.result else { return None };
    Some((report, paths))
}

/// Draws the chip while the preference is on and the last automatic check, for the inputs on
/// screen, found conflicts; nothing otherwise. It sits before the toolbar's right-hand group
/// (the caller spaces it), so it appearing moves no other button. Hovering it says what it warns about; a click opens the popover below it, and another
/// closes it. The chip and its popover have fixed ids (there is one per window), so neither
/// depends on how many widgets the toolbar drew before them.
pub fn indicator(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some((report, paths)) = shown(st) else { return };
    let Some(snap) = &st.snap else { return };
    let t = st.settings.lang();
    let count = if paths.is_empty() { "!".to_owned() } else { paths.len().to_string() };
    let galley = ui.painter().layout_no_wrap(count, type_scale::font(type_scale::SMALL), super::needs_you::ink());
    let (rect, _) = ui.allocate_exact_size(vec2(galley.size().x + 36.0, HEIGHT), Sense::hover());
    let resp = ui.interact(rect, Id::new(CHIP_ID), Sense::click());
    let p = ui.painter();
    super::needs_you::paint_pill(p, rect, resp.hovered());
    p.galley(pos2(rect.left() + 27.0, rect.center().y - galley.size().y / 2.0), galley, super::needs_you::ink());
    let target = report.request.target.name.clone();
    let label = t.trf("conflict_check.chip.name", &[("target", &target)]);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
    let remote = cc::is_remote(snap, &report.request.target);
    let resp = resp.on_hover_text(cc::chip_tooltip(t, report, remote)).on_hover_cursor(egui::CursorIcon::PointingHand);
    let id = Id::new(POPOVER_ID);
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(id));
    }
    let idle = st.job.is_none();
    popover_below(ui, id, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_width(380.0);
        popover(ui, t, report, paths, remote, idle, msgs);
    });
}

fn popover(ui: &mut egui::Ui, t: Lang, report: &ConflictReport, paths: &[String], remote: bool, idle: bool, msgs: &mut Vec<Msg>) {
    let (ours, theirs) = (report.request.branch.as_str(), report.request.target.name.as_str());
    ui.label(type_scale::section_text(ui, t.trf("conflict_check.popover.title", &[("theirs", &theirs)])));
    if let Some(note) = cc::target_note(t, &report.request.target, ours, remote) {
        ui.label(RichText::new(note).color(color::weak()));
    }
    ui.add_space(6.0);
    if paths.is_empty() {
        ui.label(cc::headline(t, report));
    } else {
        ui.label(t.trn("conflict_check.popover.both_sides", paths.len(), &[]));
        file_list(ui, t, paths, true);
    }
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        // The chip shows only a result for the inputs on screen.
        for (label, m) in actions(t, report) {
            if ui.add(buttons::secondary(label).small().enabled(idle).why(Some(crate::state::blocked::busy(t)))).clicked() {
                ui.memory_mut(|mem| mem.close_popup());
                msgs.push(m);
            }
        }
        if ui.add(buttons::secondary(t.tr("conflict_check.check_again.button")).small()).clicked() {
            ui.memory_mut(|mem| mem.close_popup());
            msgs.push(Msg::ConflictCheck(ConflictCheckMsg::CheckNow));
        }
    });
    ui.add_space(4.0);
    let off = t.tr("conflict_check.popover.turn_off.hint");
    ui.label(RichText::new(off).size(type_scale::SMALL).color(color::faint()));
}

/// The report dialog's contents; returns true when Close was clicked. `on_head`: the report is
/// for what is checked out now (otherwise it offers no Merge or Rebase).
pub fn report_modal(
    ui: &mut egui::Ui,
    cx: Cx,
    report: &ConflictReport,
    remote: bool,
    on_head: bool,
    idle: bool,
    msgs: &mut Vec<Msg>,
) -> bool {
    let t = cx.t;
    let mut d = Dialog::new(cx, t.tr("conflict_check.dialog.title")).size(Size::Wide).resizable("conflict-report").begin(ui);
    d.body(ui, |ui| {
        if let Some(note) = cc::target_note(t, &report.request.target, &report.request.branch, remote) {
            ui.label(RichText::new(note).color(color::weak()));
            ui.add_space(4.0);
        }
        ui.label(cc::headline(t, report));
        if let Ok(Prediction::Conflicts(paths)) = &report.result
            && !paths.is_empty()
        {
            ui.add_space(4.0);
            file_list(ui, t, paths, false);
        }
        if cc::offers_actions(report) && !on_head {
            ui.add_space(8.0);
            ui.label(RichText::new(t.tr("conflict_check.head_moved")).color(color::weak()));
        }
    });
    let actions = if on_head { actions(t, report) } else { Vec::new() };
    let mut footer = Footer::new().extra(buttons::secondary(t.tr("conflict_check.check_again.button"))).close();
    for (label, _) in &actions {
        footer = footer.alt(buttons::secondary(label.as_str()).enabled(idle).why(Some(crate::state::blocked::busy(t))));
    }
    let out = d.finish(ui, footer);
    if let Some(i) = out.alt
        && let Some((_, m)) = actions.into_iter().nth(i)
    {
        msgs.push(Msg::CloseModal);
        msgs.push(m);
    }
    if out.extra == Some(0) {
        msgs.push(Msg::ConflictCheck(ConflictCheckMsg::CheckNow));
    }
    out.dismiss
}

/// The Merge and Rebase actions (only for a real merge), as (label, message).
fn actions(t: Lang, report: &ConflictReport) -> Vec<(String, Msg)> {
    if !cc::offers_actions(report) {
        return Vec::new();
    }
    let (ours, theirs) = (report.request.branch.as_str(), report.request.target.name.as_str());
    let names: [(&str, &dyn std::fmt::Display); 2] = [("ours", &ours), ("theirs", &theirs)];
    vec![
        (t.trf("conflict_check.merge.button", &names), Msg::git_cmd("Merge", merge_cmd(theirs))),
        (t.trf("conflict_check.rebase.button", &names), Msg::git_cmd("Rebase", rebase_cmd(theirs))),
    ]
}

/// The paths in monospace: in the popover (`short`) the first `POPOVER_FILES` and "and K more"
/// under them, with no scroll area of their own; in the report dialog and the operation preview
/// every path, scrolling in the dialog's body.
pub(super) fn file_list(ui: &mut egui::Ui, t: Lang, paths: &[String], short: bool) {
    let (shown, more) = cc::file_list(t, paths, short.then_some(cc::POPOVER_FILES));
    egui::Frame::new().fill(color::input()).corner_radius(radius::SM).inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        for p in &shown {
            ui.label(RichText::new(p).monospace());
        }
    });
    if let Some(more) = more {
        ui.label(RichText::new(more).color(color::weak()));
    }
}

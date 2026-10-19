//! Predicted merge conflicts: the toolbar chip (a painted warning triangle and the number of
//! conflicting files) with its popover, and the report dialog for checks the branch menus ask
//! for. Texts come from state/conflict_check.rs; the buttons send the same Merge and Rebase
//! jobs as the branch menus.

use super::dialog_frame::{Cx, Dialog, Footer};
use super::theme::color;
use super::widgets::{self, Icon, buttons};
use crate::conflict_check::ConflictCheckMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::conflict_check::{self as cc, ConflictReport, JOB_RUNNING};
use crate::ui::theme::type_scale;
use eframe::egui::{self, CornerRadius, RichText, Sense, Stroke, StrokeKind, pos2, vec2};
use trellis_core::conflict_check::Prediction;

/// The chip's height; the toolbar row is 48px.
const HEIGHT: f32 = 24.0;

/// The check the chip shows and its conflicting paths, while it is drawn.
pub fn shown(st: &AppState) -> Option<(&ConflictReport, &Vec<String>)> {
    if !st.settings.conflict_check {
        return None;
    }
    let current = cc::current_request(st.snap.as_ref()?);
    let report = st.conflict_checks.shown(current.as_ref())?;
    let Ok(Prediction::Conflicts(paths)) = &report.result else { return None };
    Some((report, paths))
}

/// Draws the chip while the preference is on and the last automatic check, for the inputs on
/// screen, found conflicts; nothing otherwise (not even its spacing, so the toolbar doesn't
/// move). A click opens the popover below it.
pub fn indicator(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let Some((report, paths)) = shown(st) else { return };
    let Some(snap) = &st.snap else { return };
    let count = if paths.is_empty() { "!".to_owned() } else { paths.len().to_string() };
    let galley = ui.painter().layout_no_wrap(count, type_scale::font(type_scale::SMALL), color::text());
    ui.add_space(8.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 36.0, HEIGHT), Sense::click());
    let amber = color::amber();
    let p = ui.painter();
    let radius = CornerRadius::same((HEIGHT / 2.0) as u8);
    p.rect_filled(rect, radius, amber.gamma_multiply(if resp.hovered() { 0.28 } else { 0.16 }));
    p.rect_stroke(rect, radius, Stroke::new(1.0_f32, amber.gamma_multiply(0.6)), StrokeKind::Inside);
    widgets::paint_icon(p, pos2(rect.left() + 15.0, rect.center().y), Icon::Warning, amber);
    p.galley(pos2(rect.left() + 27.0, rect.center().y - galley.size().y / 2.0), galley, color::text());
    let target = report.request.target.name.clone();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Conflicts with {target}")));
    let resp = resp.on_hover_text(cc::chip_tooltip(report)).on_hover_cursor(egui::CursorIcon::PointingHand);
    let id = ui.make_persistent_id("conflict-popover");
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(id));
    }
    let remote = cc::is_remote(snap, &report.request.target);
    let idle = st.job.is_none();
    egui::popup::popup_below_widget(ui, id, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_width(380.0);
        popover(ui, report, paths, remote, idle, msgs);
    });
}

fn popover(ui: &mut egui::Ui, report: &ConflictReport, paths: &[String], remote: bool, idle: bool, msgs: &mut Vec<Msg>) {
    let (ours, theirs) = (report.request.branch.as_str(), report.request.target.name.as_str());
    ui.label(type_scale::section_text(ui, format!("Conflicts with {theirs}")));
    if let Some(note) = cc::target_note(&report.request.target, ours, remote) {
        ui.label(RichText::new(note).color(color::weak()));
    }
    ui.add_space(6.0);
    if paths.is_empty() {
        ui.label(cc::headline(report));
    } else {
        ui.label(format!("{} changed on both sides:", cc::file_count(paths.len())));
        file_list(ui, paths, true);
    }
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        // The chip shows only a result for the inputs on screen.
        for (label, m) in actions(report) {
            if ui.add(buttons::secondary(label).small().enabled(idle).why(Some(JOB_RUNNING))).clicked() {
                ui.memory_mut(|mem| mem.close_popup());
                msgs.push(m);
            }
        }
        if ui.add(buttons::secondary("Check again").small()).clicked() {
            ui.memory_mut(|mem| mem.close_popup());
            msgs.push(Msg::ConflictCheck(ConflictCheckMsg::CheckNow));
        }
    });
    ui.add_space(4.0);
    ui.label(RichText::new("Turn this check off in Preferences > General.").size(type_scale::SMALL).color(color::faint()));
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
    let mut d = Dialog::new(cx, "Conflict check").resizable("conflict-report").begin(ui);
    d.body(ui, |ui| {
        if let Some(note) = cc::target_note(&report.request.target, &report.request.branch, remote) {
            ui.label(RichText::new(note).color(color::weak()));
            ui.add_space(4.0);
        }
        ui.label(cc::headline(report));
        if let Ok(Prediction::Conflicts(paths)) = &report.result
            && !paths.is_empty()
        {
            ui.add_space(4.0);
            file_list(ui, paths, false);
        }
        if cc::offers_actions(report) && !on_head {
            ui.add_space(8.0);
            ui.label(RichText::new(cc::HEAD_MOVED).color(color::weak()));
        }
    });
    let actions = if on_head { actions(report) } else { Vec::new() };
    let mut footer = Footer::new().extra(buttons::secondary("Check again")).close();
    for (label, _) in &actions {
        footer = footer.alt(buttons::secondary(label.as_str()).enabled(idle).why(Some(JOB_RUNNING)));
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
fn actions(report: &ConflictReport) -> Vec<(String, Msg)> {
    if !cc::offers_actions(report) {
        return Vec::new();
    }
    let (ours, theirs) = (report.request.branch.as_str(), report.request.target.name.as_str());
    vec![
        (format!("Merge {theirs} into {ours}"), Msg::git("Merge", &["merge", theirs])),
        (format!("Rebase {ours} onto {theirs}"), Msg::git("Rebase", &["rebase", theirs])),
    ]
}

/// The paths in monospace, at most SHOWN_FILES; in the popover in a short scroll area of its
/// own (`short`), in the report dialog in the dialog's.
fn file_list(ui: &mut egui::Ui, paths: &[String], short: bool) {
    let (shown, more) = cc::file_list(paths);
    egui::Frame::new().fill(color::input()).corner_radius(4.0).inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
        let lines = |ui: &mut egui::Ui| {
            for p in &shown {
                ui.label(RichText::new(p).monospace());
            }
        };
        if short {
            egui::ScrollArea::vertical().max_height(180.0).auto_shrink([false, true]).show(ui, lines);
        } else {
            ui.set_width(ui.available_width());
            lines(ui);
        }
    });
    // Below the scroll area, so it shows without scrolling.
    if let Some(more) = more {
        ui.label(RichText::new(more).color(color::weak()));
    }
}

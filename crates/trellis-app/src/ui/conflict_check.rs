//! Predicted merge conflicts: the toolbar chip (a painted warning triangle and the number of
//! conflicting files) with its popover, and the report dialog for checks the branch menus ask
//! for. Texts come from state/conflict_check.rs; the buttons send the same Merge and Rebase
//! jobs as the branch menus.

use super::theme::color;
use super::widgets::{self, Icon};
use crate::conflict_check::ConflictCheckMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::conflict_check::{self as cc, ConflictReport, JOB_RUNNING};
use eframe::egui::{self, CornerRadius, FontId, RichText, Sense, Stroke, StrokeKind, pos2, vec2};
use trellis_core::conflict_check::Prediction;

/// The chip's height; the toolbar row is 48px.
const HEIGHT: f32 = 24.0;

/// Draws the chip while the preference is on and the last automatic check, for the inputs on
/// screen, found conflicts; nothing otherwise (not even its spacing, so the toolbar doesn't
/// move). A click opens the popover below it.
pub fn indicator(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    if !st.settings.conflict_check {
        return;
    }
    let Some(snap) = &st.snap else { return };
    let current = cc::current_request(snap);
    let Some(report) = st.conflict_checks.shown(current.as_ref()) else { return };
    let Ok(Prediction::Conflicts(paths)) = &report.result else { return };
    let count = if paths.is_empty() { "!".to_owned() } else { paths.len().to_string() };
    let galley = ui.painter().layout_no_wrap(count, FontId::proportional(12.5), color::text());
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
    ui.label(RichText::new(format!("Conflicts with {theirs}")).strong().size(14.0));
    if let Some(note) = cc::target_note(&report.request.target, ours, remote) {
        ui.label(RichText::new(note).color(color::weak()));
    }
    ui.add_space(6.0);
    if paths.is_empty() {
        ui.label(cc::headline(report));
    } else {
        ui.label(format!("{} changed on both sides:", cc::file_count(paths.len())));
        file_list(ui, paths);
    }
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        if let Some(m) = actions(ui, report, idle) {
            ui.memory_mut(|mem| mem.close_popup());
            msgs.push(m);
        }
        if ui.button("Check again").clicked() {
            ui.memory_mut(|mem| mem.close_popup());
            msgs.push(Msg::ConflictCheck(ConflictCheckMsg::CheckNow));
        }
    });
    ui.add_space(4.0);
    ui.label(RichText::new("Turn this check off in Preferences > General.").small().color(color::faint()));
}

/// The report dialog's contents; returns true when Close was clicked.
pub fn report_modal(ui: &mut egui::Ui, report: &ConflictReport, remote: bool, idle: bool, msgs: &mut Vec<Msg>) -> bool {
    ui.heading("Conflict check");
    ui.add_space(6.0);
    if let Some(note) = cc::target_note(&report.request.target, &report.request.branch, remote) {
        ui.label(RichText::new(note).color(color::weak()));
        ui.add_space(4.0);
    }
    ui.label(cc::headline(report));
    if let Ok(Prediction::Conflicts(paths)) = &report.result
        && !paths.is_empty()
    {
        ui.add_space(4.0);
        file_list(ui, paths);
    }
    ui.add_space(12.0);
    let mut close = false;
    ui.horizontal(|ui| {
        if let Some(m) = actions(ui, report, idle) {
            msgs.push(Msg::CloseModal);
            msgs.push(m);
        }
        close = ui.button("Close").clicked();
    });
    close
}

/// The Merge and Rebase buttons (only for a real merge), off while a job runs. Returns the
/// message of the one clicked.
fn actions(ui: &mut egui::Ui, report: &ConflictReport, idle: bool) -> Option<Msg> {
    if !cc::offers_actions(report) {
        return None;
    }
    let (ours, theirs) = (report.request.branch.as_str(), report.request.target.name.as_str());
    let mut clicked = None;
    let merge = ui.add_enabled(idle, egui::Button::new(format!("Merge {theirs} into {ours}"))).on_disabled_hover_text(JOB_RUNNING);
    if merge.clicked() {
        clicked = Some(Msg::git("Merge", &["merge", theirs]));
    }
    let rebase = ui.add_enabled(idle, egui::Button::new(format!("Rebase {ours} onto {theirs}"))).on_disabled_hover_text(JOB_RUNNING);
    if rebase.clicked() {
        clicked = Some(Msg::git("Rebase", &["rebase", theirs]));
    }
    clicked
}

/// The paths in monospace, at most SHOWN_FILES, in a short scroll area.
fn file_list(ui: &mut egui::Ui, paths: &[String]) {
    let (shown, more) = cc::file_list(paths);
    egui::Frame::new().fill(color::input()).corner_radius(4.0).inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
        egui::ScrollArea::vertical().max_height(180.0).auto_shrink([false, true]).show(ui, |ui| {
            for p in &shown {
                ui.label(RichText::new(p).monospace());
            }
        });
    });
    // Below the scroll area, so it shows without scrolling.
    if let Some(more) = more {
        ui.label(RichText::new(more).color(color::weak()));
    }
}

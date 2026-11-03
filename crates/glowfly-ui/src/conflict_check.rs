//! Predicted merge conflicts: the toolbar chip (a coral needs-you Small control: the warning mark and
//! the number of conflicting files in its ink) with its popover, and the report dialog for checks the branch menus ask
//! for. Texts come from state/conflict_check.rs, the parts both share from
//! conflict_check_parts.rs; the buttons send the same Merge and Rebase jobs as the branch menus.

use super::conflict_check_parts as parts;
use super::dialog_frame::{Cx, Dialog, Footer, Size};
use super::theme::color;
use super::widgets::buttons;
use super::widgets::icon::Icon;
use super::widgets::menu::cut_name;
use super::widgets::popover;
use super::widgets::truncate::{cut_middle_in, truncate_label};
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::conflict_check::{self as cc, ConflictReport};
use crate::ui::theme::sizes::ControlSize;
use crate::ui::theme::type_scale;
use crate::ui::widgets::menu::popover_below;
use eframe::egui::{self, Align, Id, Layout, RichText, Sense};
use glowfly_core::branch_ops::{merge_cmd, rebase_cmd};
use glowfly_core::conflict_check::{Prediction, TargetKind};
use glowfly_core::i18n::Lang;
use glowfly_model::conflict_check::ConflictCheckMsg;

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
    let (rect, _) = ui.allocate_exact_size(super::needs_you::control_size(galley.size().x), Sense::hover());
    let resp = ui.interact(rect, Id::new(CHIP_ID), Sense::click());
    super::needs_you::paint_control(ui.painter(), rect, resp.hovered(), galley);
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
    popover_below(ui, id, &resp, POPOVER_W, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        popover(ui, t, report, paths, remote, idle, msgs);
    });
}

/// The popover's width with its frame (narrower only in a narrower window).
const POPOVER_W: f32 = 420.0;
/// The gap between the popover's groups, and between buttons.
const GROUP_GAP: f32 = 16.0;
const BUTTON_GAP: f32 = 8.0;
/// The header's needs-you mark.
const MARK: f32 = 16.0;
/// The widest the default branch's name gets in an action's label.
const NAME_W: f32 = 120.0;
/// The actions' tooltip wrap width: the owner's command on one line.
const TIP_W: f32 = 520.0;

/// The popover (issue #523's design): a header row with the needs-you mark, the title and Check
/// again; the names block; the file list; Merge and Rebase under short labels with what they do
/// and the command on hover; and a quiet footer under a divider saying where to turn the
/// check off. Every gap is set here, on the 8 px grid.
fn popover(ui: &mut egui::Ui, t: Lang, report: &ConflictReport, paths: &[String], remote: bool, idle: bool, msgs: &mut Vec<Msg>) {
    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
    header(ui, t, report, msgs);
    ui.add_space(8.0);
    parts::names(ui, t, report, remote);
    ui.add_space(GROUP_GAP);
    if paths.is_empty() {
        ui.add(egui::Label::new(cc::headline(t, report)).wrap());
    } else {
        parts::file_list(ui, t, paths, true);
    }
    // The chip shows only a result for the inputs on screen.
    let actions = actions(ui, t, report);
    if !actions.is_empty() {
        ui.add_space(GROUP_GAP);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(BUTTON_GAP, BUTTON_GAP);
            for a in actions {
                let b = buttons::secondary(a.label.as_str())
                    .tip(&a.tip)
                    .tip_width(TIP_W)
                    .enabled(idle)
                    .why(Some(crate::state::blocked::busy(t)));
                if ui.add(b).clicked() {
                    ui.memory_mut(|mem| mem.close_popup());
                    msgs.push(a.msg);
                }
            }
        });
    }
    ui.add_space(GROUP_GAP);
    parts::divider(ui, f32::from(popover::PAD));
    ui.add_space(12.0);
    let off = t.tr("conflict_check.popover.turn_off.hint");
    ui.add(egui::Label::new(RichText::new(off).size(type_scale::SMALL).color(color::faint())).wrap());
}

/// The popover's header row: the needs-you mark, the title (what the branch conflicts with, cut
/// at its end if a translation runs long) and Check again as an icon button at the right.
fn header(ui: &mut egui::Ui, t: Lang, report: &ConflictReport, msgs: &mut Vec<Msg>) {
    let title = match report.request.target.kind {
        TargetKind::Upstream => t.tr("conflict_check.popover.title.upstream"),
        TargetKind::DefaultBranch => t.tr("conflict_check.popover.title.default"),
        TargetKind::Chosen => t.tr("conflict_check.dialog.title"),
    };
    let w = ui.available_width();
    ui.allocate_ui_with_layout(egui::vec2(w, parts::ROW_H), Layout::left_to_right(Align::Center), |ui| {
        let (mark, _) = ui.allocate_exact_size(egui::vec2(MARK, MARK), Sense::hover());
        super::needs_you::badge(ui.painter(), mark);
        ui.add_space(8.0);
        let room = ui.available_width() - 8.0 - parts::ROW_H;
        truncate_label(ui, type_scale::section_text(ui, title), room);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let again = buttons::icon_only(Icon::Refresh, t.tr("conflict_check.check_again.button")).size(ControlSize::Small);
            if ui.add(again).clicked() {
                ui.memory_mut(|mem| mem.close_popup());
                msgs.push(Msg::ConflictCheck(ConflictCheckMsg::CheckNow));
            }
        });
    });
}

/// The report dialog's contents; returns true when Close was clicked. `on_head`: the report is
/// for what is checked out now (otherwise it offers no Merge or Rebase). Its body matches the
/// popover's: the names block, then the file list (every path) or the result's sentence.
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
        ui.spacing_mut().item_spacing.y = 0.0;
        parts::names(ui, t, report, remote);
        ui.add_space(GROUP_GAP);
        if let Ok(Prediction::Conflicts(paths)) = &report.result {
            ui.add(egui::Label::new(RichText::new(t.tr("conflict_check.dialog.conflicts")).color(color::text())).wrap());
            if !paths.is_empty() {
                ui.add_space(GROUP_GAP);
                parts::file_list(ui, t, paths, false);
            }
        } else {
            ui.add(egui::Label::new(cc::headline(t, report)).wrap());
        }
        if cc::offers_actions(report) && !on_head {
            ui.add_space(GROUP_GAP);
            ui.add(egui::Label::new(RichText::new(t.tr("conflict_check.head_moved")).color(color::weak())).wrap());
        }
    });
    let actions = if on_head { actions(ui, t, report) } else { Vec::new() };
    let mut footer = Footer::new().extra(buttons::secondary(t.tr("conflict_check.check_again.button"))).close();
    for a in &actions {
        footer = footer
            .alt(buttons::secondary(a.label.as_str()).tip(&a.tip).tip_width(TIP_W).enabled(idle).why(Some(crate::state::blocked::busy(t))));
    }
    let out = d.finish(ui, footer);
    if let Some(i) = out.alt
        && let Some(a) = actions.into_iter().nth(i)
    {
        msgs.push(Msg::CloseModal);
        msgs.push(a.msg);
    }
    if out.extra == Some(0) {
        msgs.push(Msg::ConflictCheck(ConflictCheckMsg::CheckNow));
    }
    out.dismiss
}

/// A Merge or Rebase action: its button label, its hover text (what it does, then the git
/// command) and the message it sends.
struct Action {
    label: String,
    tip: String,
    msg: Msg,
}

/// The Merge and Rebase actions (only for a real merge). Against the upstream they take short
/// labels that name no branch, against the default branch its name cut in its middle to
/// `NAME_W`, and against a branch the user picked the full sentence with each name cut. The hover
/// text says what the action does and, on its own line, the git command it runs.
fn actions(ui: &egui::Ui, t: Lang, report: &ConflictReport) -> Vec<Action> {
    if !cc::offers_actions(report) {
        return Vec::new();
    }
    let (ours, theirs) = (report.request.branch.as_str(), report.request.target.name.as_str());
    let named = cut_middle_in(ui.painter(), theirs, &type_scale::font(type_scale::BODY), NAME_W);
    let names: [(&str, &dyn std::fmt::Display); 2] = [("ours", &ours), ("theirs", &theirs)];
    let (cut_ours, cut_theirs) = (cut_name(ours), cut_name(theirs));
    let cut: [(&str, &dyn std::fmt::Display); 2] = [("ours", &cut_ours), ("theirs", &cut_theirs)];
    let name: [(&str, &dyn std::fmt::Display); 1] = [("name", &named)];
    let kind = report.request.target.kind;
    let merge = match kind {
        TargetKind::Upstream => t.tr("conflict_check.merge.short.upstream").to_owned(),
        TargetKind::DefaultBranch => t.trf("conflict_check.merge.short.named", &name),
        TargetKind::Chosen => t.trf("conflict_check.merge.button", &cut),
    };
    let rebase = match kind {
        TargetKind::Upstream => t.tr("conflict_check.rebase.short.upstream").to_owned(),
        TargetKind::DefaultBranch => t.trf("conflict_check.rebase.short.named", &name),
        TargetKind::Chosen => t.trf("conflict_check.rebase.button", &cut),
    };
    let (merge_says, rebase_says) = match kind {
        TargetKind::Chosen => {
            let runs = t.tr("conflict_check.runs");
            let merge = t.trf("conflict_check.merge.button", &names);
            let rebase = t.trf("conflict_check.rebase.button", &names);
            (format!("{merge} {runs}"), format!("{rebase} {runs}"))
        }
        TargetKind::Upstream | TargetKind::DefaultBranch => {
            (t.tr("conflict_check.merge.tip").to_owned(), t.tr("conflict_check.rebase.tip").to_owned())
        }
    };
    let action = |label: String, says: String, verb: &str, cmd: Vec<String>| {
        let tip = format!("{says}\ngit {}", cmd.join(" "));
        Action { label, tip, msg: Msg::git_cmd(verb, cmd) }
    };
    vec![action(merge, merge_says, "Merge", merge_cmd(theirs)), action(rebase, rebase_says, "Rebase", rebase_cmd(theirs))]
}

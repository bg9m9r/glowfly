//! The interactive rebase panel, shown in place of the graph: a header with Restore original
//! order (Reload steps on a stale list), Cancel and
//! Start rebase (Update steps, on a running rebase's remaining steps), why it is unavailable,
//! the list in the order git runs it (`rows`, with `commit_row`, `step_row`, `editor`, `drag`
//! and `keys`), and buttons that add exec and break steps. Also the graph menus' history edits
//! (`edit_menu`), the Reword dialog (`reword_dialog`) and the rebase banner on the WIP row
//! (`banner`).

pub(super) mod banner;
mod commit_row;
mod drag;
pub(super) mod edit_menu;
mod editor;
mod keys;
mod merge_row;
pub(super) mod reword_dialog;
mod rows;
mod step_row;

use super::UiCache;
use super::theme::color;
use super::widgets::{Icon, paint_icon};
use crate::msg::Msg;
use crate::msg::RebaseMsg;
use crate::state::AppState;
use crate::state::rebase::{HINT, PanelSource, RebasePanel, needs_stash};
use crate::state::rebase_remaining::{NO_STEPS, REMAINING_HINT, REMAINING_TITLE, UPDATE};
use crate::state::rebase_steps::NewStep;
use crate::ui::theme::type_scale;
use crate::ui::widgets::{buttons, truncate};
use eframe::egui::{self, Align, Layout, RichText, Sense, Stroke, Ui, vec2};
use trellis_core::i18n::Lang;

const STASH_HINT: &str = "Uncommitted changes will be stashed first and put back afterwards.";

pub fn show(ui: &mut Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(panel) = &st.rebase else { return };
    let remaining = panel.source == PanelSource::Remaining;
    let blocker = st.snap.as_ref().and_then(|s| panel.start_blocker(s, st.job.is_some()));
    ui.add_space(8.0);
    header(ui, st.settings.lang(), panel, blocker.as_deref(), msgs);
    ui.add_space(4.0);
    egui::Frame::new().inner_margin(egui::Margin { left: 14, right: 14, top: 0, bottom: 4 }).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        if panel.editable()
            && let Some(why) = &blocker
        {
            ui.label(RichText::new(why).color(color::removed()));
        }
        if let Some(e) = panel.error.as_ref().filter(|e| blocker.as_ref() != Some(*e)) {
            ui.add(egui::Label::new(RichText::new(e).color(color::removed())).wrap());
        }
        ui.label(RichText::new(if remaining { REMAINING_HINT } else { HINT }).size(type_scale::SMALL).color(color::weak()));
        if !remaining && st.snap.as_ref().is_some_and(needs_stash) {
            ui.label(RichText::new(STASH_HINT).size(type_scale::SMALL).color(color::weak()));
        }
    });
    ui.separator();
    if panel.loading {
        ui.add_space(24.0);
        ui.vertical_centered(|ui| ui.add(egui::Spinner::new().size(22.0)));
        return;
    }
    rows::list(ui, st, cache, msgs);
}

/// Title and subtitle on the left (the subtitle cut with "…" before the buttons); Restore
/// original order, Cancel and Start rebase (or Update steps) on the right.
fn header(ui: &mut Ui, t: Lang, panel: &RebasePanel, blocker: Option<&str>, msgs: &mut Vec<Msg>) {
    let (title, primary) =
        if panel.source == PanelSource::Remaining { (REMAINING_TITLE, UPDATE) } else { ("Interactive rebase", "Start rebase") };
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        // The buttons first, from the right, so the title and subtitle get what is left.
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(14.0);
            if ui.add(buttons::primary(primary).enabled(blocker.is_none()).why(blocker)).clicked() {
                msgs.push(Msg::Rebase(RebaseMsg::StartRebase { confirmed: false, autostash: false }));
            }
            let cancel =
                buttons::secondary(t.tr("common.cancel.button")).enabled(!panel.starting).tip("Close without changing anything (Esc)");
            if ui.add(cancel).clicked() {
                msgs.push(Msg::Rebase(RebaseMsg::CloseRebase));
            }
            let (label, tip) = if panel.stale {
                ("Reload steps", "Reload the list from the repository")
            } else {
                ("Restore original order", "Back to the original order and actions; typed messages and added steps are dropped")
            };
            if ui.add(buttons::secondary(label).enabled(panel.editable()).tip(tip)).clicked() {
                msgs.push(Msg::Rebase(RebaseMsg::RebaseReset));
            }
            if panel.starting {
                ui.add(egui::Spinner::new());
            }
            ui.add_space(8.0);
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let w = ui.available_width();
                let font = type_scale::strong(ui, type_scale::SUBHEAD);
                truncate::truncate_label(ui, RichText::new(title).font(font).color(color::text()), w);
                truncate::truncate_label(ui, RichText::new(panel.subtitle()).color(color::weak()), w);
            });
        });
    });
}

/// Under the list: Add exec step and Add break step (after the selected row), and the note
/// for a running rebase with no steps left.
fn add_buttons(ui: &mut Ui, panel: &RebasePanel, msgs: &mut Vec<Msg>) {
    if panel.source == PanelSource::Remaining && panel.items.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(rows::MARGIN + 4.0);
            ui.label(RichText::new(NO_STEPS).color(color::weak()));
        });
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(rows::MARGIN);
        ui.add_enabled_ui(panel.editable(), |ui| {
            for (label, step) in [("Add exec step", NewStep::Exec), ("Add break step", NewStep::Break)] {
                if plus_button(ui, label).clicked() {
                    msgs.push(Msg::Rebase(RebaseMsg::RebaseAddStep { after: panel.selected, step }));
                }
            }
        });
    });
}

/// A button with a painted plus before its text (the fonts lack some glyphs).
fn plus_button(ui: &mut Ui, label: &str) -> egui::Response {
    let enabled = ui.is_enabled();
    let text_color = if enabled { color::text() } else { color::faint() };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), egui::TextStyle::Button.resolve(ui.style()), text_color);
    let size = vec2(galley.size().x + 38.0, 28.0);
    let (rect, r) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    let p = ui.painter();
    let fill = if enabled && r.hovered() { color::hover() } else { color::raised() };
    p.rect_filled(rect, egui::CornerRadius::same(4), fill);
    p.rect_stroke(rect, egui::CornerRadius::same(4), Stroke::new(1.0_f32, color::border()), egui::StrokeKind::Inside);
    paint_icon(p, egui::pos2(rect.left() + 15.0, rect.center().y), Icon::Plus, if enabled { color::primary() } else { color::faint() });
    p.galley(egui::pos2(rect.left() + 28.0, rect.center().y - galley.size().y / 2.0), galley, text_color);
    r
}

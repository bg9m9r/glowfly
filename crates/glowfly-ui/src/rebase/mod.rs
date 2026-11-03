//! The interactive rebase panel, shown in place of the graph: a header with ⋯ (Restore original
//! order), Cancel and Start rebase (Update steps, on a running rebase's remaining steps; Reload
//! steps on a stale list), why it is unavailable,
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
use crate::ui::theme::tokens::radius;
use crate::ui::theme::type_scale;
use crate::ui::widgets::menu::{self, popup_below};
use crate::ui::widgets::{buttons, truncate};
use eframe::egui::{self, Align, Layout, PopupCloseBehavior, RichText, Sense, Stroke, Ui, vec2};
use glowfly_core::i18n::Lang;

const STASH_HINT: &str = "Uncommitted changes will be stashed first and put back afterwards.";

pub fn show(ui: &mut Ui, st: &mut AppState, cache: &mut UiCache, msgs: &mut Vec<Msg>) {
    let Some(panel) = &st.session.rebase.view else { return };
    let remaining = panel.source == PanelSource::Remaining;
    let blocker = st.snap.as_ref().and_then(|s| panel.start_blocker(s, st.job.is_some()));
    let blocker = blocker.map(|b| panel.held_text(st.settings.lang(), &b).unwrap_or(b));
    let blocker = blocker.or_else(|| st.snap.as_ref().and_then(|s| panel.message_blocker(s, st.settings.lang())));
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

/// Title and subtitle on the left (the subtitle cut with "…" before the buttons); at the
/// trailing edge ⋯ (Restore original order), Cancel and the one filled button: Start rebase
/// (Update steps on a running rebase's remaining steps), or Reload steps on a stale list, which
/// hides the ⋯.
fn header(ui: &mut Ui, t: Lang, panel: &RebasePanel, blocker: Option<&str>, msgs: &mut Vec<Msg>) {
    let (title, primary) =
        if panel.source == PanelSource::Remaining { (REMAINING_TITLE, UPDATE) } else { ("Interactive rebase", "Start rebase") };
    let action = if panel.stale {
        buttons::primary("Reload steps").enabled(panel.editable()).tip("Reload the list from the repository")
    } else {
        buttons::primary(primary).enabled(blocker.is_none()).why(blocker)
    };
    let cancel = buttons::secondary(t.tr("common.cancel.button")).enabled(!panel.starting).tip("Close without changing anything (Esc)");
    let more_button = (!panel.stale).then(|| buttons::icon_only(Icon::More, t.tr("rebase.more.name")).enabled(panel.editable()));
    // Top-aligned: the buttons line up with the title, as before the order changed.
    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
        ui.add_space(14.0);
        // Laid out in reading order (so Tab follows it), the title and subtitle taking the room
        // the buttons at the trailing edge leave.
        let gap = ui.spacing().item_spacing.x;
        let spinner = if panel.starting { SPINNER + gap } else { 0.0 };
        let more_w = more_button.as_ref().map_or(0.0, |b| b.desired_width(ui) + gap);
        let trailing = spinner + more_w + cancel.desired_width(ui) + gap + action.desired_width(ui);
        let title_w = (ui.available_width() - 14.0 - 8.0 - gap - trailing).max(0.0);
        ui.allocate_ui_with_layout(vec2(title_w, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(title_w);
            ui.spacing_mut().item_spacing.y = 2.0;
            let font = type_scale::strong(ui, type_scale::SUBHEAD);
            truncate::truncate_label(ui, RichText::new(title).font(font).color(color::text()), title_w);
            truncate::truncate_label(ui, RichText::new(panel.subtitle()).color(color::weak()), title_w);
        });
        ui.add_space(8.0);
        if panel.starting {
            ui.add(egui::Spinner::new().size(SPINNER));
        }
        if let Some(b) = more_button {
            more(ui, t, panel, b, msgs);
        }
        if ui.add(cancel).clicked() {
            msgs.push(Msg::Rebase(RebaseMsg::CloseRebase));
        }
        if ui.add(action).clicked() {
            msgs.push(Msg::Rebase(if panel.stale {
                RebaseMsg::RebaseReset
            } else {
                RebaseMsg::StartRebase { confirmed: false, autostash: false }
            }));
        }
    });
}

/// The spinner beside the buttons while the rebase starts.
const SPINNER: f32 = 16.0;

/// The header's ⋯ and its menu: Restore original order, disabled while nothing has changed.
fn more(ui: &mut Ui, t: Lang, panel: &RebasePanel, button: buttons::StyledButton<'_>, msgs: &mut Vec<Msg>) {
    let id = ui.make_persistent_id("rebase-more");
    let r = ui.add(button);
    if r.clicked() {
        ui.memory_mut(|m| m.toggle_popup(id));
    }
    popup_below(ui, id, &r, PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(220.0);
        let why = (panel.items == panel.original).then(|| t.tr("rebase.restore.unchanged"));
        let item = menu::item(t.tr("rebase.restore.button")).blocked(why).show(ui).on_hover_text(t.tr("rebase.restore.tip"));
        if item.clicked() && why.is_none() {
            msgs.push(Msg::Rebase(RebaseMsg::RebaseReset));
            ui.memory_mut(|m| m.close_popup());
        }
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
    p.rect_filled(rect, egui::CornerRadius::same(radius::SM), fill);
    p.rect_stroke(rect, egui::CornerRadius::same(radius::SM), Stroke::new(1.0_f32, color::border()), egui::StrokeKind::Inside);
    let plus = if !enabled {
        color::faint()
    } else if r.hovered() {
        color::text()
    } else {
        color::weak()
    };
    paint_icon(p, egui::pos2(rect.left() + 15.0, rect.center().y), Icon::Plus, plus);
    p.galley(egui::pos2(rect.left() + 28.0, rect.center().y - galley.size().y / 2.0), galley, text_color);
    r
}

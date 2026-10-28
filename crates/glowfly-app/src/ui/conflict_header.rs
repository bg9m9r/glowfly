//! The conflict view's header: the path and the operation on one row; under it, from the right
//! edge inwards, Close, the more-actions menu (Save with markers, Open in merge tool, Reset
//! picks…, Mark resolved), the primary Save and mark resolved, how many conflicts are resolved,
//! and the conflict counter with previous/next. Why Save is disabled goes to its left when the
//! row has room for it after the counter and the steps, else into the path's row, ending above
//! Save. With no merge tool set, Open in merge tool stays enabled ("Set up…") and opens
//! Preferences. Files with no chunk view get only Close and the menu.

use super::super::access;
use super::super::common::{menu_item_if, menu_item_setup};
use super::super::theme::color;
use super::super::widgets::{self, Icon, buttons, icon_button, reason};
use super::step_tip;
use crate::keymap::Keymap;
use crate::msg::{ConflictMsg, Msg};
use crate::state::conflict::ConflictView;
use crate::tools::ToolsMsg;
use crate::ui::theme::type_scale;
use eframe::egui::{self, Align, Layout, RichText, Sense, TextStyle, Ui, UiBuilder, WidgetText, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{ConflictContent, Operation, SideLabels};

/// The widest Save's reason gets before it is cut.
const SAVE_REASON_W: f32 = 260.0;
/// The space between the progress (or "No conflicting lines") and what is right of it.
const STEPS_GAP: f32 = 10.0;
/// A painted step arrow's width (`icon_button`).
const STEP_W: f32 = 26.0;

/// What the open file is in the middle of: "Merging feature into main".
fn operation_text(t: Lang, l: &SideLabels) -> String {
    let sides: [(&str, &dyn std::fmt::Display); 2] = [("ours", &l.ours), ("theirs", &l.theirs)];
    match l.operation {
        Operation::Merge => t.trf("conflict.header.operation.merge", &sides),
        Operation::Rebase => t.trf("conflict.header.operation.rebase", &sides),
        Operation::CherryPick => t.trf("conflict.header.operation.cherry_pick", &sides),
        Operation::Revert => t.trf("conflict.header.operation.revert", &sides),
        Operation::Other => t.trf("conflict.header.operation.applying", &sides),
    }
}

/// The merge tool entry: why it can't open (if it can't), and its setup note while no merge
/// tool is set (it then opens Preferences > External Tools).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct MergeTool<'a> {
    pub blocked: Option<&'a str>,
    pub setup: Option<&'static str>,
}

/// The header's two rows, with the merge `tool` entry in the menu; nothing writes the file while
/// `in_tool`.
pub(super) fn header_bar(ui: &mut Ui, t: Lang, view: &ConflictView, tool: MergeTool, in_tool: bool, keys: &Keymap, msgs: &mut Vec<Msg>) {
    // The path's row is drawn after the buttons, which decide whether Save's reason moves up
    // into it.
    let path_font = type_scale::strong(ui, type_scale::SUBHEAD);
    let row_h = ui.fonts(|f| f.row_height(&path_font)).max(ui.spacing().interact_size.y);
    let (top, _) = ui.allocate_exact_size(vec2(ui.available_width(), row_h), Sense::hover());
    let mut moved = None;
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| moved = header_buttons(ui, t, view, tool, in_tool, keys, msgs));
    });
    let mut row = ui.new_child(UiBuilder::new().max_rect(top).layout(Layout::right_to_left(Align::Center)));
    if let Some((why, save_right)) = moved {
        row.add_space((top.right() - save_right).max(0.0));
        let max = SAVE_REASON_W.min(row.available_width() / 2.0);
        reason::beside(&mut row, &why, max);
    }
    row.with_layout(Layout::left_to_right(Align::Center), |ui| {
        ui.add_space(8.0);
        // Cut short rather than run past the panel.
        ui.add(egui::Label::new(RichText::new(&view.path).font(path_font).color(color::text())).truncate());
        if let Some(f) = &view.file {
            ui.add(egui::Label::new(RichText::new(operation_text(t, &f.labels)).color(color::weak())).truncate());
        }
    });
}

/// The header's buttons, laid out right to left. Returns Save's reason when the row has no room
/// for it, with Save's right edge, for the path's row.
fn header_buttons(
    ui: &mut Ui,
    t: Lang,
    view: &ConflictView,
    tool: MergeTool,
    in_tool: bool,
    keys: &Keymap,
    msgs: &mut Vec<Msg>,
) -> Option<(String, f32)> {
    if ui.add(buttons::secondary(t.tr("conflict.header.close.button"))).clicked() {
        msgs.push(Msg::Conflict(ConflictMsg::CloseConflict));
    }
    let text = view.file.as_ref().is_some_and(|f| matches!(f.content, ConflictContent::Text(_)));
    more_menu(ui, t, view, text, tool, in_tool, msgs);
    if !text {
        return None;
    }
    let can = view.can_save() && !in_tool;
    let why = if in_tool {
        t.tr("tools.in_merge_tool").to_owned()
    } else if view.saving {
        t.tr("conflict.header.saving.tip").to_owned()
    } else {
        t.trf("conflict.header.save.pick_first.tip", &[("left", &view.unresolved())])
    };
    let save = ui.add(buttons::primary(t.tr("conflict.header.save.button")).enabled(can).why(Some(&why)));
    let save = if can { save.on_hover_text(t.tr("conflict.header.save.tip")) } else { save };
    if save.clicked() {
        msgs.push(Msg::Conflict(ConflictMsg::SaveConflict));
    }
    let n = view.chunk_count();
    let done = view.picked();
    let progress = t.trn("conflict.header.progress", n, &[("done", &done)]);
    let counter = if n == 1 {
        t.tr("conflict.header.counter.single").to_owned()
    } else {
        t.trf("conflict.header.counter.of", &[("current", &(view.current + 1)), ("total", &n)])
    };
    let mut moved = None;
    if !can {
        // Right to left: left of Save, in the room the progress and the steps leave; when that
        // is too little, up in the path's row instead of pushing them out of the header.
        let room = ui.available_width() - steps_width(ui, t, n, &progress, &counter) - ui.spacing().item_spacing.x;
        if reason::fits_in_row(room) {
            reason::beside(ui, &why, SAVE_REASON_W.min(room));
        } else {
            moved = Some((why, save.rect.right()));
        }
    }
    if n == 0 {
        ui.add_space(STEPS_GAP);
        ui.label(RichText::new(t.tr("conflict.header.no_lines")).color(color::weak()));
        return moved;
    }
    // A longer translation can leave too little room: the progress gives way (the Output
    // strip shows it too) rather than pushing the steps out of the header.
    if ui.available_width() >= steps_width(ui, t, n, &progress, &counter) {
        let tint = if done == n { color::added() } else { color::weak() };
        ui.label(RichText::new(progress).color(tint));
        ui.add_space(STEPS_GAP);
    }
    // Right to left: next first. Arrows are painted (the bundled fonts lack them).
    let tip = step_tip(keys, t, true);
    let next = ui.add_enabled_ui(view.current + 1 < n, |ui| icon_button(ui, Icon::NextChange, &tip)).inner;
    if next.clicked() {
        msgs.push(Msg::Conflict(ConflictMsg::ConflictStep(1)));
    }
    ui.label(RichText::new(counter).color(color::text()));
    let tip = step_tip(keys, t, false);
    let prev = ui.add_enabled_ui(view.current > 0, |ui| icon_button(ui, Icon::PrevChange, &tip)).inner;
    if prev.clicked() {
        msgs.push(Msg::Conflict(ConflictMsg::ConflictStep(-1)));
    }
    moved
}

/// The width the row needs left of Save's reason: the progress, the next arrow, the counter and
/// the previous arrow, or "No conflicting lines" when there are no chunks.
fn steps_width(ui: &Ui, t: Lang, n: usize, progress: &str, counter: &str) -> f32 {
    let sp = ui.spacing().item_spacing.x;
    let text_w = |s: &str| WidgetText::from(s).into_galley(ui, None, f32::INFINITY, TextStyle::Body).size().x;
    if n == 0 {
        return STEPS_GAP + sp + text_w(t.tr("conflict.header.no_lines"));
    }
    text_w(progress) + STEPS_GAP + 2.0 * STEP_W + text_w(counter) + 4.0 * sp
}

/// The painted more-actions button and its menu, each item disabled with its reason. Save with
/// markers and Reset picks… are for text files (`text`) only.
fn more_menu(ui: &mut Ui, t: Lang, view: &ConflictView, text: bool, tool: MergeTool, in_tool: bool, msgs: &mut Vec<Msg>) {
    let path = &view.path;
    let menu = crate::ui::widgets::menu::custom_button(ui, egui::Button::new("").min_size(vec2(28.0, 24.0)), |ui| {
        if text {
            menu_item_if(
                msgs,
                ui,
                t.tr("conflict.header.markers.button"),
                markers_blocked(t, view, in_tool),
                Msg::Conflict(ConflictMsg::SaveConflictKeepMarkers),
            );
        }
        let open = Msg::Tools(ToolsMsg::ResolveInMergeTool(path.clone()));
        match tool.blocked {
            Some(why) => menu_item_if(msgs, ui, t.tr("tools.merge_tool.button"), Some(why), open),
            None => menu_item_setup(msgs, ui, t, t.tr("tools.merge_tool.button"), tool.setup, open),
        }
        if text {
            let blocked = if in_tool {
                Some(t.tr("tools.in_merge_tool"))
            } else if view.saving {
                Some(t.tr("conflict.header.saving.tip"))
            } else {
                None
            };
            menu_item_if(msgs, ui, t.tr("conflict.header.reset.button"), blocked, view.reset_msg(t));
        }
        let blocked = in_tool.then(|| t.tr("tools.in_merge_tool"));
        menu_item_if(
            msgs,
            ui,
            t.tr("conflict.header.mark_resolved.button"),
            blocked,
            Msg::Conflict(ConflictMsg::MarkResolved(vec![path.clone()])),
        );
    });
    let r = menu.response.on_hover_text(t.tr("conflict.header.more.tip"));
    widgets::paint_icon(ui.painter(), r.rect.center(), Icon::More, color::text());
    access::name_button(&r, true, t.tr("conflict.header.more.tip"));
}

/// Why Save with markers can't run: it needs something picked or edited and something left.
fn markers_blocked(t: Lang, view: &ConflictView, in_tool: bool) -> Option<&'static str> {
    if in_tool {
        Some(t.tr("tools.in_merge_tool"))
    } else if view.saving {
        Some(t.tr("conflict.header.saving.tip"))
    } else if view.unresolved() == 0 {
        Some(t.tr("conflict.header.markers.all_picked.tip"))
    } else if !view.can_save_partial() {
        Some(t.tr("conflict.header.markers.pick_first.tip"))
    } else {
        None
    }
}

#[cfg(test)]
#[path = "snapshot_tests/snapshot_tests_conflict_header.rs"]
mod snapshot_tests;

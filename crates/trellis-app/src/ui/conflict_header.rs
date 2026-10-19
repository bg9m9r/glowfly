//! The conflict view's header: the path and the operation on one row; under it, from the right
//! edge inwards, Close, the more-actions menu (Save with markers, Open in merge tool, Reset
//! picks…, Mark resolved), the primary Save and mark resolved, how many conflicts are resolved,
//! and the conflict counter with previous/next. Files with no chunk view get only Close and the
//! menu.

use super::super::access;
use super::super::common::menu_item_if;
use super::super::theme::color;
use super::super::widgets::{self, Icon, buttons, icon_button};
use super::step_tip;
use crate::keymap::Keymap;
use crate::msg::{ConflictMsg, Msg};
use crate::state::conflict::ConflictView;
use crate::state::tool_compare::{IN_MERGE_TOOL, MERGE_TOOL_BUTTON};
use crate::tools::ToolsMsg;
use eframe::egui::{self, Align, Layout, RichText, Ui, vec2};
use trellis_core::i18n::Lang;
use trellis_core::repo::{ConflictContent, Operation, SideLabels};

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

/// The header's two rows. `tool` is why the merge tool can't open (if it can't); nothing writes
/// the file while `in_tool`.
pub(super) fn header_bar(ui: &mut Ui, t: Lang, view: &ConflictView, tool: Option<&str>, in_tool: bool, keys: &Keymap, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        // Cut short rather than run past the panel.
        ui.add(egui::Label::new(RichText::new(&view.path).strong().size(15.0).color(color::text())).truncate());
        if let Some(f) = &view.file {
            ui.add(egui::Label::new(RichText::new(operation_text(t, &f.labels)).color(color::weak())).truncate());
        }
    });
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| header_buttons(ui, t, view, tool, in_tool, keys, msgs));
    });
}

/// The header's buttons, laid out right to left.
fn header_buttons(ui: &mut Ui, t: Lang, view: &ConflictView, tool: Option<&str>, in_tool: bool, keys: &Keymap, msgs: &mut Vec<Msg>) {
    if ui.add(buttons::secondary(t.tr("conflict.header.close.button"))).clicked() {
        msgs.push(Msg::Conflict(ConflictMsg::CloseConflict));
    }
    let text = view.file.as_ref().is_some_and(|f| matches!(f.content, ConflictContent::Text(_)));
    more_menu(ui, t, view, text, tool, in_tool, msgs);
    if !text {
        return;
    }
    let can = view.can_save() && !in_tool;
    let why = if in_tool {
        IN_MERGE_TOOL.to_owned()
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
    if n == 0 {
        ui.add_space(10.0);
        ui.label(RichText::new(t.tr("conflict.header.no_lines")).color(color::weak()));
        return;
    }
    let done = view.picked();
    let tint = if done == n { color::green() } else { color::weak() };
    ui.label(RichText::new(t.trn("conflict.header.progress", n, &[("done", &done)])).color(tint));
    ui.add_space(10.0);
    // Right to left: next first. Arrows are painted (the bundled fonts lack them).
    let tip = step_tip(keys, true);
    let next = ui.add_enabled_ui(view.current + 1 < n, |ui| icon_button(ui, Icon::NextChange, &tip)).inner;
    if next.clicked() {
        msgs.push(Msg::Conflict(ConflictMsg::ConflictStep(1)));
    }
    let counter = if n == 1 {
        t.tr("conflict.header.counter.single").to_owned()
    } else {
        t.trf("conflict.header.counter.of", &[("current", &(view.current + 1)), ("total", &n)])
    };
    ui.label(RichText::new(counter).color(color::text()));
    let tip = step_tip(keys, false);
    let prev = ui.add_enabled_ui(view.current > 0, |ui| icon_button(ui, Icon::PrevChange, &tip)).inner;
    if prev.clicked() {
        msgs.push(Msg::Conflict(ConflictMsg::ConflictStep(-1)));
    }
}

/// The painted more-actions button and its menu, each item disabled with its reason. Save with
/// markers and Reset picks… are for text files (`text`) only.
fn more_menu(ui: &mut Ui, t: Lang, view: &ConflictView, text: bool, tool: Option<&str>, in_tool: bool, msgs: &mut Vec<Msg>) {
    let path = &view.path;
    let menu = egui::menu::menu_custom_button(ui, egui::Button::new("").min_size(vec2(28.0, 24.0)), |ui| {
        if text {
            menu_item_if(
                msgs,
                ui,
                t.tr("conflict.header.markers.button"),
                markers_blocked(t, view, in_tool),
                Msg::Conflict(ConflictMsg::SaveConflictKeepMarkers),
            );
        }
        menu_item_if(msgs, ui, MERGE_TOOL_BUTTON, tool, Msg::Tools(ToolsMsg::ResolveInMergeTool(path.clone())));
        if text {
            let blocked = if in_tool {
                Some(IN_MERGE_TOOL)
            } else if view.saving {
                Some(t.tr("conflict.header.saving.tip"))
            } else {
                None
            };
            menu_item_if(msgs, ui, t.tr("conflict.header.reset.button"), blocked, view.reset_msg(t));
        }
        let blocked = in_tool.then_some(IN_MERGE_TOOL);
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
        Some(IN_MERGE_TOOL)
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
#[path = "snapshot_tests_conflict_header.rs"]
mod snapshot_tests;

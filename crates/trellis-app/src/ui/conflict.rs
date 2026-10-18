//! The conflict view: ours and theirs side by side with picks per conflict chunk (whole sides
//! or single lines), and below them, in a panel whose height can be dragged, the output as it
//! will be saved (editable by hand). Long unchanged runs fold in both, and long lines wrap with
//! the diff viewer's Wrap long lines setting. Binary, deleted and other special files offer a
//! whole version instead.

use super::conflict_output::output;
use super::conflict_panes::panes;
use super::theme::color;
use super::widgets::{Icon, icon_button};
use crate::keymap::{BindingId, Keymap};
use crate::msg::ConflictMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::conflict::ConflictView;
use crate::state::panel_layout::{CONFLICT_OUTPUT, CONFLICT_PANES_MIN};
use crate::state::tool_compare::{IN_MERGE_TOOL, MERGE_TOOL_BUTTON, merge_blocked, merging};
use crate::tools::ToolsMsg;
use eframe::egui::{self, Align, Color32, Frame, Layout, RichText, Ui};
use trellis_core::repo::{ConflictContent, ConflictFile, Operation, Side, SideLabels};

pub fn show(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let scroll_to = st.view.conflict_scroll_to.take();
    let wrap = st.settings.diff_word_wrap;
    let Some(view) = &st.conflict else { return };
    let tool = merge_blocked(&st.settings.external_tools, view.file.as_ref().map(|f| &f.content), st.job.is_some());
    // Nothing here writes the file while the merge tool has it open; it closes the view on a change.
    let in_tool = merging(st, &view.path);
    ui.add_space(6.0);
    header_bar(ui, view, tool.as_deref(), in_tool, &st.keymap, msgs);
    ui.separator();
    if in_tool {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new(format!("{IN_MERGE_TOOL}.")).color(color::amber()));
        });
    }
    auto_note(ui, view);
    restored_note(ui, view);
    let Some(file) = &view.file else {
        ui.label(RichText::new("Loading…").color(color::weak()));
        return;
    };
    if !matches!(file.content, ConflictContent::Text(_)) {
        fallback(ui, view, file, tool.as_deref(), in_tool, msgs);
        return;
    }
    ui.spacing_mut().item_spacing.y = 0.0;
    // The output first, at the bottom (its height kept by egui for the session); the panes take
    // the rest. Its editor changes its own text buffer.
    let avail = ui.available_height();
    egui::TopBottomPanel::bottom("conflict-output")
        .resizable(true)
        .default_height(CONFLICT_OUTPUT.default_for(avail).floor())
        .height_range(CONFLICT_OUTPUT.min..=(avail - CONFLICT_PANES_MIN).max(CONFLICT_OUTPUT.min))
        .frame(Frame::new().fill(color::bg()))
        .show_inside(ui, |ui| {
            if let Some(view) = st.conflict.as_mut() {
                output(ui, view, scroll_to, wrap, msgs);
            }
        });
    egui::CentralPanel::default().frame(Frame::new()).show_inside(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        if let Some(view) = &st.conflict
            && let Some(file) = &view.file
            && let ConflictContent::Text(segs) = &file.content
        {
            panes(ui, view, file, segs, scroll_to, wrap, msgs);
        }
    });
}

fn operation_text(l: &SideLabels) -> String {
    let (ours, theirs) = (&l.ours, &l.theirs);
    match l.operation {
        Operation::Merge => format!("Merging {theirs} into {ours}"),
        Operation::Rebase => format!("Rebasing {theirs} onto {ours}"),
        Operation::CherryPick => format!("Cherry-picking {theirs} onto {ours}"),
        Operation::Revert => format!("Reverting {theirs} on {ours}"),
        Operation::Other => format!("Applying changes onto {ours}"),
    }
}

/// The path and operation on one row; under it, at the right (from the edge inwards) Close,
/// Mark resolved, Open in merge tool (disabled with the reason `tool` when set), Save, Save
/// with markers, Reset and the conflict counter with previous/next. Mark resolved and both
/// saves are disabled while `in_tool`.
fn header_bar(ui: &mut Ui, view: &ConflictView, tool: Option<&str>, in_tool: bool, keys: &Keymap, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        // Cut short rather than run past the panel.
        ui.add(egui::Label::new(RichText::new(&view.path).strong().size(15.0).color(color::text())).truncate());
        if let Some(f) = &view.file {
            ui.add(egui::Label::new(RichText::new(operation_text(&f.labels)).color(color::weak())).truncate());
        }
    });
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| header_buttons(ui, view, tool, in_tool, keys, msgs));
    });
}

/// The tooltip of the next (`forward`) or previous conflict arrow, with the step's current key.
pub(super) fn step_tip(keys: &Keymap, forward: bool) -> String {
    if forward { keys.tip("Next conflict", BindingId::NextChange) } else { keys.tip("Previous conflict", BindingId::PrevChange) }
}

/// The header's buttons, laid out right to left.
fn header_buttons(ui: &mut Ui, view: &ConflictView, tool: Option<&str>, in_tool: bool, keys: &Keymap, msgs: &mut Vec<Msg>) {
    if ui.button("Close  (Esc)").clicked() {
        msgs.push(Msg::Conflict(ConflictMsg::CloseConflict));
    }
    mark_resolved_button(ui, &view.path, in_tool, msgs);
    merge_tool_button(ui, &view.path, tool, msgs);
    if !view.file.as_ref().is_some_and(|f| matches!(f.content, ConflictContent::Text(_))) {
        return;
    }
    let can = view.can_save() && !in_tool;
    let save = egui::Button::new(RichText::new("Save").strong().color(if can { Color32::WHITE } else { color::faint() })).fill(if can {
        color::primary()
    } else {
        color::toolbar()
    });
    let why = if in_tool {
        IN_MERGE_TOOL.to_owned()
    } else if view.saving {
        "Saving…".to_owned()
    } else {
        format!("Pick every conflict first ({} left), or use Save with markers", view.unresolved())
    };
    if ui.add_enabled(can, save).on_hover_text("Write the output to the file and mark it resolved").on_disabled_hover_text(why).clicked() {
        msgs.push(Msg::Conflict(ConflictMsg::SaveConflict));
    }
    save_partial_button(ui, view, in_tool, msgs);
    if ui.button("Reset").on_hover_text("Clear all picks (typed text stays)").clicked() {
        msgs.push(Msg::Conflict(ConflictMsg::ConflictReset));
    }
    ui.add_space(10.0);
    let n = view.chunk_count();
    if n == 0 {
        ui.label(RichText::new("No conflicting lines").color(color::weak()));
        return;
    }
    // Right to left: next first. Arrows are painted (the bundled fonts lack them).
    let tip = step_tip(keys, true);
    let next = ui.add_enabled_ui(view.current + 1 < n, |ui| icon_button(ui, Icon::NextChange, &tip)).inner;
    if next.clicked() {
        msgs.push(Msg::Conflict(ConflictMsg::ConflictStep(1)));
    }
    let counter = if n == 1 { "1 conflict".to_owned() } else { format!("conflict {} of {n}", view.current + 1) };
    ui.label(RichText::new(counter).color(color::text()));
    let tip = step_tip(keys, false);
    let prev = ui.add_enabled_ui(view.current > 0, |ui| icon_button(ui, Icon::PrevChange, &tip)).inner;
    if prev.clicked() {
        msgs.push(Msg::Conflict(ConflictMsg::ConflictStep(-1)));
    }
}

/// Save with markers: writes the output with conflict markers for the conflicts not picked
/// yet, leaving the file conflicted. Needs something picked or edited and something left.
fn save_partial_button(ui: &mut Ui, view: &ConflictView, in_tool: bool, msgs: &mut Vec<Msg>) {
    let why = if in_tool {
        Some(IN_MERGE_TOOL)
    } else if view.saving {
        Some("Saving…")
    } else if view.unresolved() == 0 {
        Some("Every conflict is picked: use Save")
    } else if !view.can_save_partial() {
        Some("Pick or edit a conflict first")
    } else {
        None
    };
    let tip = "Write the output to the file with conflict markers for the conflicts not picked yet; the file stays conflicted";
    let r = ui.add_enabled(why.is_none(), egui::Button::new("Save with markers"));
    if r.on_hover_text(tip).on_disabled_hover_text(why.unwrap_or_default()).clicked() {
        msgs.push(Msg::Conflict(ConflictMsg::SaveConflictKeepMarkers));
    }
}

/// Mark resolved for `path`, disabled while the merge tool has it open (`in_tool`).
fn mark_resolved_button(ui: &mut Ui, path: &str, in_tool: bool, msgs: &mut Vec<Msg>) {
    let r = ui.add_enabled(!in_tool, egui::Button::new("Mark resolved"));
    let tip = "Stage the working file as it is now (asks first if conflict markers are left)";
    if r.on_hover_text(tip).on_disabled_hover_text(IN_MERGE_TOOL).clicked() {
        msgs.push(Msg::Conflict(ConflictMsg::MarkResolved(vec![path.to_owned()])));
    }
}

/// Under the header: how many chunks were picked automatically, while any pick still is.
fn auto_note(ui: &mut Ui, view: &ConflictView) {
    let (auto, n) = (view.auto_count(), view.chunk_count());
    if auto == 0 {
        return;
    }
    let text = if auto == n {
        let all = if n == 1 { "The conflict in this file was" } else { "All conflicts in this file were" };
        RichText::new(format!(
            "{all} picked automatically (one side unchanged, or only whitespace differs). You can save it straight away."
        ))
        .color(color::accent())
    } else {
        RichText::new(format!(
            "{auto} of {n} conflicts were picked automatically (one side unchanged, or only whitespace differs). Check them before \
             saving."
        ))
        .color(color::weak())
    };
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(text);
    });
}

/// Under the header: how many chunks came back from the file saved with markers, or that its
/// edits between the conflicts did, when anything did.
fn restored_note(ui: &mut Ui, view: &ConflictView) {
    let text = match (view.restored, view.chunk_count()) {
        (0, _) if view.baseline.1.is_empty() => return,
        (0, _) => "Edits saved in the file between the conflicts are back.".to_owned(),
        (k, n) => format!("Picks saved in the file are back for {k} of {n} conflicts."),
    };
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new(text).color(color::accent()));
    });
}

/// Open in merge tool for `path`, disabled with the reason `blocked`.
fn merge_tool_button(ui: &mut Ui, path: &str, blocked: Option<&str>, msgs: &mut Vec<Msg>) {
    let r = ui.add_enabled(blocked.is_none(), egui::Button::new(MERGE_TOOL_BUTTON));
    let r = r.on_hover_text("Resolve the conflict in the external merge tool").on_disabled_hover_text(blocked.unwrap_or_default());
    if r.clicked() {
        msgs.push(Msg::Tools(ToolsMsg::ResolveInMergeTool(path.to_owned())));
    }
}

/// Files with no chunk view: the reason, and buttons to take a whole version (and for a binary
/// file, to open the merge tool, disabled with the reason `tool`). Nothing writes while `in_tool`.
fn fallback(ui: &mut Ui, view: &ConflictView, file: &ConflictFile, tool: Option<&str>, in_tool: bool, msgs: &mut Vec<Msg>) {
    let (labels, content) = (&file.labels, &file.content);
    let message = match content {
        ConflictContent::Binary => "Binary file: pick a whole version.".to_owned(),
        ConflictContent::DeletedBy(Side::Theirs) => format!("Deleted on the theirs side ({})", labels.theirs),
        ConflictContent::DeletedBy(Side::Ours) => format!("Deleted on the ours side ({})", labels.ours),
        ConflictContent::Unsupported(reason) => format!("This file can't be merged here: {reason}."),
        ConflictContent::Text(_) => return,
    };
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new(message).size(14.0).color(color::text()));
    });
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        let path = &view.path;
        for (side, word, name) in [(Side::Ours, "ours", &labels.ours), (Side::Theirs, "theirs", &labels.theirs)] {
            let deleted = *content == ConflictContent::DeletedBy(side);
            let (label, body) = if deleted {
                (format!("Delete file ({name})"), format!("Delete {path} and mark it resolved? Changes in the working file are lost."))
            } else {
                (
                    format!("Use {word} ({name})"),
                    format!("Replace {path} with the {word} version and mark it resolved? Changes in the working file are lost."),
                )
            };
            let r = ui.add_enabled(!view.saving && !in_tool, egui::Button::new(&label));
            if (if in_tool { r.on_disabled_hover_text(IN_MERGE_TOOL) } else { r }).clicked() {
                let resolve = Msg::Conflict(ConflictMsg::ResolveWithSide { path: path.clone(), side });
                msgs.push(Msg::confirm("Resolve conflict", body, if deleted { "Delete" } else { "Replace" }, resolve));
            }
        }
        mark_resolved_button(ui, path, in_tool, msgs);
        if *content == ConflictContent::Binary {
            merge_tool_button(ui, path, tool, msgs);
        }
    });
}

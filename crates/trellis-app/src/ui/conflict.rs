//! The conflict view: ours and theirs side by side with picks per conflict chunk (whole sides
//! or single lines), and below them, in a panel whose height can be dragged, the output as it
//! will be saved (editable by hand). Long unchanged runs fold in both, and long lines wrap with
//! the diff viewer's Wrap long lines setting. Binary, deleted and other special files offer a
//! whole version instead.

#[path = "conflict_header.rs"]
mod header;

use super::conflict_output::output;
use super::conflict_panes::panes;
use super::theme::color;
use crate::keymap::{BindingId, Keymap};
use crate::msg::ConflictMsg;
use crate::msg::Msg;
use crate::state::AppState;
use crate::state::conflict::ConflictView;
use crate::state::panel_layout::{CONFLICT_OUTPUT, CONFLICT_PANES_MIN};
use crate::state::tool_compare::{IN_MERGE_TOOL, merge_blocked, merging};
use eframe::egui::{self, Frame, RichText, Ui};
use trellis_core::repo::{ConflictContent, ConflictFile, Side};

const LOADING: &str = "Loading the conflict…";

pub fn show(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let scroll_to = st.view.conflict_scroll_to.take();
    let wrap = st.settings.diff_word_wrap;
    let Some(view) = &st.conflict else { return };
    let tool = merge_blocked(&st.settings.external_tools, view.file.as_ref().map(|f| &f.content), st.job.is_some());
    // Nothing here writes the file while the merge tool has it open; it closes the view on a change.
    let in_tool = merging(st, &view.path);
    ui.add_space(6.0);
    header::header_bar(ui, st.settings.lang(), view, tool.as_deref(), in_tool, &st.keymap, msgs);
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
        return super::pane_status::loading(ui, LOADING);
    };
    if !matches!(file.content, ConflictContent::Text(_)) {
        fallback(ui, view, file, in_tool, msgs);
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

/// The tooltip of the next (`forward`) or previous conflict arrow, with the step's current key.
pub(super) fn step_tip(keys: &Keymap, forward: bool) -> String {
    if forward { keys.tip("Next conflict", BindingId::NextChange) } else { keys.tip("Previous conflict", BindingId::PrevChange) }
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

/// Files with no chunk view: the reason, and buttons to take a whole version (the header's menu
/// holds Open in merge tool and Mark resolved). Nothing writes while `in_tool`.
fn fallback(ui: &mut Ui, view: &ConflictView, file: &ConflictFile, in_tool: bool, msgs: &mut Vec<Msg>) {
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
                msgs.push(Msg::confirm_danger("Resolve conflict", body, if deleted { "Delete" } else { "Replace" }, resolve));
            }
        }
    });
}

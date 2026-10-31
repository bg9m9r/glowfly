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
use crate::state::tool_compare::{ToolSetup, merge_blocked, merging};
use crate::ui::theme::type_scale;
use eframe::egui::{self, Frame, RichText, Ui};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{ConflictContent, ConflictFile, Side, Unmergeable};

pub fn show(ui: &mut Ui, st: &mut AppState, msgs: &mut Vec<Msg>) {
    let scroll_to = st.view.conflict_scroll_to.take();
    let wrap = st.settings.diff_word_wrap;
    let t = st.settings.lang();
    let Some(view) = &st.session.conflict.view else { return };
    let blocked = merge_blocked(t, view.file.as_ref().map(|f| &f.content), st.job.is_some());
    let setup = ToolSetup::of(t, &st.settings.external_tools).merge;
    // Nothing here writes the file while the merge tool has it open; it closes the view on a change.
    let in_tool = merging(st, &view.path);
    ui.add_space(6.0);
    let tool = header::MergeTool { blocked: blocked.as_deref(), setup };
    header::header_bar(ui, t, view, tool, in_tool, &st.keymap, msgs);
    ui.separator();
    if in_tool {
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(RichText::new(t.tr("conflict.view.in_tool.note")).color(color::warn()));
        });
    }
    auto_note(ui, t, view);
    restored_note(ui, t, view);
    let Some(file) = &view.file else {
        return super::pane_status::loading(ui, t.tr("conflict.view.loading"));
    };
    if !matches!(file.content, ConflictContent::Text(_)) {
        fallback(ui, t, view, file, in_tool, msgs);
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
            if let Some(view) = st.session.conflict.view.as_mut() {
                output(ui, t, view, scroll_to, wrap, msgs);
            }
        });
    egui::CentralPanel::default().frame(Frame::new()).show_inside(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        if let Some(view) = &st.session.conflict.view
            && let Some(file) = &view.file
            && let ConflictContent::Text(segs) = &file.content
        {
            panes(ui, t, view, file, segs, scroll_to, wrap, msgs);
        }
    });
}

/// The tooltip of the next (`forward`) or previous conflict arrow, with the step's current key.
pub(super) fn step_tip(keys: &Keymap, t: Lang, forward: bool) -> String {
    if forward {
        keys.tip(t.tr("conflict.view.next.tip"), BindingId::NextChange)
    } else {
        keys.tip(t.tr("conflict.view.prev.tip"), BindingId::PrevChange)
    }
}

/// Under the header: how many chunks were picked automatically, while any pick still is.
fn auto_note(ui: &mut Ui, t: Lang, view: &ConflictView) {
    let (auto, n) = (view.auto_count(), view.chunk_count());
    if auto == 0 {
        return;
    }
    let text = if auto == n {
        let all = if n == 1 { t.tr("conflict.view.auto_one") } else { t.tr("conflict.view.auto_all") };
        RichText::new(all).color(color::accent())
    } else {
        RichText::new(t.trn("conflict.view.auto_some", n, &[("auto", &auto)])).color(color::weak())
    };
    // Wraps, so a longer translation can't widen the view.
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.add(egui::Label::new(text).wrap());
    });
}

/// Under the header: how many chunks came back from the file saved with markers, or that its
/// edits between the conflicts did, when anything did.
fn restored_note(ui: &mut Ui, t: Lang, view: &ConflictView) {
    let text = match (view.restored, view.chunk_count()) {
        (0, _) if view.baseline.1.is_empty() => return,
        (0, _) => t.tr("conflict.view.restored_edits").to_owned(),
        (k, n) => t.trn("conflict.view.restored_picks", n, &[("restored", &k)]),
    };
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.add(egui::Label::new(RichText::new(text).color(color::accent())).wrap());
    });
}

/// Files with no chunk view: the reason, and buttons to take a whole version (the header's menu
/// holds Open in merge tool and Mark resolved). Nothing writes while `in_tool`.
fn fallback(ui: &mut Ui, t: Lang, view: &ConflictView, file: &ConflictFile, in_tool: bool, msgs: &mut Vec<Msg>) {
    let (labels, content) = (&file.labels, &file.content);
    let message = match content {
        ConflictContent::Binary => t.tr("conflict.view.binary").to_owned(),
        ConflictContent::DeletedBy(Side::Theirs) => t.trf("conflict.view.deleted_theirs", &[("name", &labels.theirs)]),
        ConflictContent::DeletedBy(Side::Ours) => t.trf("conflict.view.deleted_ours", &[("name", &labels.ours)]),
        ConflictContent::Unsupported(Unmergeable::SymbolicLink) => t.tr("conflict.view.unsupported.symlink").to_owned(),
        ConflictContent::Unsupported(Unmergeable::Submodule) => t.tr("conflict.view.unsupported.submodule").to_owned(),
        ConflictContent::Unsupported(Unmergeable::NotUtf8) => t.tr("conflict.view.unsupported.not_utf8").to_owned(),
        ConflictContent::Text(_) => return,
    };
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new(message).size(type_scale::BODY).color(color::text()));
    });
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        let path = &view.path;
        for (side, name) in [(Side::Ours, &labels.ours), (Side::Theirs, &labels.theirs)] {
            let deleted = *content == ConflictContent::DeletedBy(side);
            let name: [(&str, &dyn std::fmt::Display); 1] = [("name", name)];
            let path: [(&str, &dyn std::fmt::Display); 1] = [("path", path)];
            let (label, body, button) = match (deleted, side) {
                (true, _) => (
                    t.trf("conflict.view.delete_file.button", &name),
                    t.trf("conflict.view.delete_file.body", &path),
                    t.tr("common.delete.button"),
                ),
                (false, Side::Ours) => (
                    t.trf("conflict.view.use_ours.button", &name),
                    t.trf("conflict.view.use_ours.body", &path),
                    t.tr("conflict.view.replace.button"),
                ),
                (false, Side::Theirs) => (
                    t.trf("conflict.view.use_theirs.button", &name),
                    t.trf("conflict.view.use_theirs.body", &path),
                    t.tr("conflict.view.replace.button"),
                ),
            };
            let r = ui.add_enabled(!view.saving && !in_tool, egui::Button::new(&label));
            if (if in_tool { r.on_disabled_hover_text(t.tr("tools.in_merge_tool")) } else { r }).clicked() {
                let resolve = Msg::Conflict(ConflictMsg::ResolveWithSide { path: view.path.clone(), side });
                msgs.push(Msg::confirm_danger(t.tr("conflict.view.resolve.title"), body, button, resolve));
            }
        }
    });
}

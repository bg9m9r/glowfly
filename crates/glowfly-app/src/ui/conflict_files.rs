//! The WIP panel's Conflicted Files and Resolved Files lists, above Unstaged Files while an
//! operation is in progress: conflicted files with their marker counts and Mark all resolved,
//! and resolved files with Mark conflicted (one or all).

use super::common::menu_item;
use super::details_wip::{OpenFile, RowContext, wip_file_row};
use super::file_menu;
use super::theme::color;
use super::widgets::{Icon, paint_icon};
use crate::msg::{ConflictMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::conflict_files::{mark_conflicted_body, restorable_paths};
use crate::state::path_hazards::hazard_of;
use crate::state::tool_compare::merging;
use crate::ui::theme::tokens::radius;
use crate::ui::theme::type_scale;
use eframe::egui::{self, Align, Layout, Sense, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{DiffTarget, FileEntry, Hazard, ResolvedFile, Snapshot};

/// The rows of both lists and the height each may take.
pub(super) struct Lists<'a> {
    pub conflicted: &'a [&'a FileEntry],
    pub resolved: &'a [&'a ResolvedFile],
    pub height: f32,
}

/// Draws the lists that have rows (neither, while nothing is conflicted or resolved).
pub(super) fn show(ui: &mut egui::Ui, st: &AppState, lists: Lists, open: &OpenFile, ctx: RowContext, msgs: &mut Vec<Msg>) {
    let Some(snap) = &st.snap else { return };
    if !lists.conflicted.is_empty() {
        conflicted(ui, st, snap, &lists, open, ctx, msgs);
        ui.add_space(8.0);
    }
    if !lists.resolved.is_empty() {
        resolved(ui, st, snap, &lists, open, msgs);
        ui.add_space(8.0);
    }
}

/// A list header like Unstaged Files': the title with its count, and `buttons` at the right.
fn header(ui: &mut egui::Ui, title: String, buttons: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new().fill(color::raised()).inner_margin(egui::Margin::symmetric(8, 5)).corner_radius(radius::SM).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.add(egui::Label::new(type_scale::section_text(ui, title)).selectable(false));
            ui.with_layout(Layout::right_to_left(Align::Center), buttons);
        });
    });
}

fn conflicted(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, lists: &Lists, open: &OpenFile, ctx: RowContext, msgs: &mut Vec<Msg>) {
    let (rows, t) = (lists.conflicted, st.settings.lang());
    let usable: Vec<String> = rows.iter().filter(|f| !matches!(f.hazard, Some(Hazard::NotUtf8(_)))).map(|f| f.path.clone()).collect();
    header(ui, t.trf("conflict.files.conflicted.header", &[("count", &rows.len())]), |ui| {
        let b = ui.add_enabled(!usable.is_empty(), egui::Button::new(t.tr("conflict.files.mark_all_resolved.button")).small());
        if b.on_hover_text(t.tr("conflict.files.mark_all_resolved.tip")).clicked() {
            msgs.push(Msg::Conflict(ConflictMsg::MarkResolved(usable.clone())));
        }
    });
    let area = egui::ScrollArea::vertical().id_salt("Conflicted files").max_height(lists.height).auto_shrink([false, true]);
    area.show_rows(ui, super::widgets::LIST_ROW_H, rows.len(), |ui, range| {
        for f in &rows[range] {
            let _ = wip_file_row(ui, snap, f, false, open, ctx, st.conflict_files.count_note(t, &f.path), msgs);
        }
    });
}

fn resolved(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, lists: &Lists, open: &OpenFile, msgs: &mut Vec<Msg>) {
    let (rows, t) = (lists.resolved, st.settings.lang());
    header(ui, t.trf("conflict.files.resolved.header", &[("count", &rows.len())]), |ui| {
        let all = restorable_paths(st);
        let why = rows.iter().find_map(|r| blocked(st, t, r)).unwrap_or_else(|| t.tr("conflict.files.nothing_restorable.tip").into());
        let b = ui.add_enabled(!all.is_empty(), egui::Button::new(t.tr("conflict.files.mark_all_conflicted.button")).small());
        if b.on_hover_text(t.tr("conflict.files.mark_all_conflicted.tip")).on_disabled_hover_text(why).clicked() {
            let then = Msg::Conflict(ConflictMsg::MarkConflicted(all.clone()));
            let (title, button) = (t.tr("conflict.files.mark_conflicted.title"), t.tr("conflict.files.mark_conflicted.button"));
            msgs.push(Msg::confirm(title, mark_conflicted_body(t, &all), button, then));
        }
    });
    let area = egui::ScrollArea::vertical().id_salt("Resolved files").max_height(lists.height).auto_shrink([false, true]);
    area.show_rows(ui, super::widgets::LIST_ROW_H, rows.len(), |ui, range| {
        for r in &rows[range] {
            resolved_row(ui, st, snap, r, open, msgs);
        }
    });
}

/// Why Mark conflicted is off for `r`, or None.
fn blocked(st: &AppState, t: Lang, r: &ResolvedFile) -> Option<String> {
    let path = &r.entry.path;
    if let Some(h) = r.entry.hazard.as_ref().or_else(|| hazard_of(st, path)) {
        return Some(h.reason(t));
    }
    if !r.both_sides {
        return Some(t.tr("conflict.files.both_sides.tip").into());
    }
    merging(st, path).then(|| t.tr("tools.in_merge_tool").to_owned())
}

/// A resolved file: a check, the path (opens its staged diff when it has one) and Mark
/// conflicted, which asks first.
fn resolved_row(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, r: &ResolvedFile, open: &OpenFile, msgs: &mut Vec<Msg>) {
    let (path, t) = (&r.entry.path, st.settings.lang());
    let why = blocked(st, t, r);
    let mark = t.tr("conflict.files.mark_conflicted.button");
    let confirm = || {
        let body = mark_conflicted_body(t, std::slice::from_ref(path));
        let then = Msg::Conflict(ConflictMsg::MarkConflicted(vec![path.clone()]));
        Msg::confirm(t.tr("conflict.files.mark_conflicted.title"), body, mark, then)
    };
    let staged = snap.staged.iter().any(|f| f.path == *path);
    let selected = matches!(open.diff, Some(DiffTarget::Staged(p)) if p == path);
    let row = super::widgets::file_row(ui, selected, path, |ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
        paint_icon(ui.painter(), rect.center(), Icon::Check, color::weak());
        super::widgets::row_text(ui, egui::RichText::new(path).monospace(), selected);
        super::path_warnings::mark(ui, t, &r.entry);
        let b = ui.add_enabled(why.is_none(), egui::Button::new(mark).small());
        let tip = t.tr("conflict.files.mark_conflicted.tip");
        let b = b.on_hover_text(tip).on_disabled_hover_text(why.as_deref().unwrap_or_default());
        if b.clicked() {
            msgs.push(confirm());
        }
    })
    .response;
    if row.clicked() && staged {
        msgs.push(Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Staged(path.clone()))));
    }
    row.context_menu(|ui| {
        super::common::menu_item_if(msgs, ui, mark, why.as_deref(), confirm());
        menu_item(msgs, ui, t.tr("common.copy_path"), Msg::Copy(path.clone()));
        if r.entry.hazard.is_none() {
            file_menu::history_items(ui, t, msgs, path, None, None);
        }
    });
}

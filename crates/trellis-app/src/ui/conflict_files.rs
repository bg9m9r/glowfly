//! The WIP panel's Conflicted Files and Resolved Files lists, above Unstaged Files while an
//! operation is in progress: conflicted files with their marker counts and Mark all resolved,
//! and resolved files with Mark conflicted (one or all).

use super::common::{menu_item, widget_row_height};
use super::details_wip::{OpenFile, RowContext, wip_file_row};
use super::file_menu;
use super::theme::color;
use super::widgets::{Icon, paint_icon};
use crate::msg::{ConflictMsg, Msg, RepoMsg};
use crate::state::AppState;
use crate::state::conflict_files::{mark_conflicted_body, restorable_paths};
use crate::state::path_hazards::hazard_of;
use crate::state::tool_compare::{IN_MERGE_TOOL, merging};
use eframe::egui::{self, Align, Layout, RichText, Sense, vec2};
use trellis_core::repo::{DiffTarget, FileEntry, Hazard, ResolvedFile, Snapshot};

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
fn header(ui: &mut egui::Ui, title: &str, n: usize, buttons: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new().fill(color::toolbar()).inner_margin(egui::Margin::symmetric(8, 5)).corner_radius(3).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.add(egui::Label::new(RichText::new(format!("{title} ({n})")).strong().color(color::text())).selectable(false));
            ui.with_layout(Layout::right_to_left(Align::Center), buttons);
        });
    });
}

fn conflicted(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, lists: &Lists, open: &OpenFile, ctx: RowContext, msgs: &mut Vec<Msg>) {
    let rows = lists.conflicted;
    let usable: Vec<String> = rows.iter().filter(|f| !matches!(f.hazard, Some(Hazard::NotUtf8(_)))).map(|f| f.path.clone()).collect();
    header(ui, "Conflicted Files", rows.len(), |ui| {
        let b = ui.add_enabled(!usable.is_empty(), egui::Button::new("Mark all resolved").small());
        if b.on_hover_text("Stage every conflicted file as it is now").clicked() {
            msgs.push(Msg::Conflict(ConflictMsg::MarkResolved(usable.clone())));
        }
    });
    let area = egui::ScrollArea::vertical().id_salt("Conflicted Files").max_height(lists.height).auto_shrink([false, true]);
    area.show_rows(ui, widget_row_height(ui), rows.len(), |ui, range| {
        for f in &rows[range] {
            wip_file_row(ui, snap, f, false, open, ctx, st.conflict_files.count_note(&f.path), msgs);
        }
    });
}

fn resolved(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, lists: &Lists, open: &OpenFile, msgs: &mut Vec<Msg>) {
    let rows = lists.resolved;
    header(ui, "Resolved Files", rows.len(), |ui| {
        let all = restorable_paths(st);
        let why = rows.iter().find_map(|r| blocked(st, r)).unwrap_or_else(|| "Nothing here can be marked conflicted".into());
        let b = ui.add_enabled(!all.is_empty(), egui::Button::new("Mark all conflicted").small());
        if b.on_hover_text("Bring the conflicts of every resolved file back").on_disabled_hover_text(why).clicked() {
            let then = Msg::Conflict(ConflictMsg::MarkConflicted(all.clone()));
            msgs.push(Msg::confirm("Mark conflicted", mark_conflicted_body(&all), "Mark conflicted", then));
        }
    });
    let area = egui::ScrollArea::vertical().id_salt("Resolved Files").max_height(lists.height).auto_shrink([false, true]);
    area.show_rows(ui, widget_row_height(ui), rows.len(), |ui, range| {
        for r in &rows[range] {
            resolved_row(ui, st, snap, r, open, msgs);
        }
    });
}

/// Why Mark conflicted is off for `r`, or None.
fn blocked(st: &AppState, r: &ResolvedFile) -> Option<String> {
    let path = &r.entry.path;
    if let Some(h) = r.entry.hazard.as_ref().or_else(|| hazard_of(st, path)) {
        return Some(h.reason());
    }
    if !r.both_sides {
        return Some("git can only restore a conflict that has both versions".into());
    }
    merging(st, path).then(|| IN_MERGE_TOOL.to_owned())
}

/// A resolved file: a check, the path (opens its staged diff when it has one) and Mark
/// conflicted, which asks first.
fn resolved_row(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, r: &ResolvedFile, open: &OpenFile, msgs: &mut Vec<Msg>) {
    let path = &r.entry.path;
    let why = blocked(st, r);
    let confirm = || {
        let body = mark_conflicted_body(std::slice::from_ref(path));
        Msg::confirm("Mark conflicted", body, "Mark conflicted", Msg::Conflict(ConflictMsg::MarkConflicted(vec![path.clone()])))
    };
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
        paint_icon(ui.painter(), rect.center(), Icon::Check, color::green());
        let staged = snap.staged.iter().any(|f| f.path == *path);
        let selected = matches!(open.diff, Some(DiffTarget::Staged(p)) if p == path);
        let label = ui.selectable_label(selected, path);
        super::path_warnings::mark(ui, &r.entry);
        if label.clicked() && staged {
            msgs.push(Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Staged(path.clone()))));
        }
        let b = ui.add_enabled(why.is_none(), egui::Button::new("Mark conflicted").small());
        let b = b.on_hover_text("Bring the conflict back into the file").on_disabled_hover_text(why.as_deref().unwrap_or_default());
        if b.clicked() {
            msgs.push(confirm());
        }
        label.context_menu(|ui| {
            super::common::menu_item_if(msgs, ui, "Mark conflicted", why.as_deref(), confirm());
            menu_item(msgs, ui, "Copy path", Msg::Copy(path.clone()));
            if r.entry.hazard.is_none() {
                file_menu::history_items(ui, msgs, path, None, None);
            }
        });
    });
}

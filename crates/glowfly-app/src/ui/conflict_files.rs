//! The WIP panel's Conflicted Files and Resolved Files lists, in step 1 of an operation in
//! progress (`op_steps`): conflicted files with their marker counts and Mark resolved at the
//! row's end (Mark all resolved in the header), and resolved files with Mark conflicted (one
//! or all).

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
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use crate::ui::widgets::row_menu::RowMenuExt;
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

/// A list header's one action: its label, why it is off (or its tip), and what it sends.
struct Action<'a> {
    label: &'a str,
    tip: &'a str,
    why: Option<String>,
    msg: Msg,
}

/// A list header like Unstaged Files': the title with its count, and `action` at the right as a
/// quiet button, or in a more-actions menu when the two don't fit side by side (a narrow panel,
/// a long language).
fn header(ui: &mut egui::Ui, t: Lang, title: String, action: Action, msgs: &mut Vec<Msg>) {
    super::details_wip::header::frame(ui, |ui| {
        let b = buttons::quiet(action.label).small().enabled(action.why.is_none());
        let b = if action.why.is_some() { b.why(action.why.as_deref()) } else { b.tip(action.tip) };
        let font = type_scale::section(ui);
        let title_w = ui.fonts(|f| f.layout_no_wrap(title.clone(), font, egui::Color32::PLACEHOLDER).size().x);
        if b.desired_width(ui) + ui.spacing().item_spacing.x + title_w <= ui.available_width() {
            if ui.add(b).clicked() {
                msgs.push(action.msg);
            }
        } else {
            super::details_wip::header::more_menu(ui, t, |ui| {
                super::common::menu_item_if(msgs, ui, action.label, action.why.as_deref(), action.msg);
            });
        }
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.add(egui::Label::new(type_scale::section_text(ui, title)).selectable(false).truncate());
        });
    });
}

fn conflicted(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, lists: &Lists, open: &OpenFile, ctx: RowContext, msgs: &mut Vec<Msg>) {
    let (rows, t) = (lists.conflicted, st.settings.lang());
    let usable: Vec<String> = rows.iter().filter(|f| !matches!(f.hazard, Some(Hazard::NotUtf8(_)))).map(|f| f.path.clone()).collect();
    let action = Action {
        label: t.tr("conflict.files.mark_all_resolved.button"),
        tip: t.tr("conflict.files.mark_all_resolved.tip"),
        why: usable.is_empty().then(|| rows.iter().find_map(|f| f.hazard.as_ref()).map(|h| h.reason(t))).flatten(),
        msg: Msg::Conflict(ConflictMsg::MarkResolved(usable.clone())),
    };
    header(ui, t, t.trf("conflict.files.conflicted.header", &[("count", &rows.len())]), action, msgs);
    let area = egui::ScrollArea::vertical().id_salt("Conflicted files").max_height(lists.height).auto_shrink([false, true]);
    area.show_rows(ui, super::widgets::LIST_ROW_H, rows.len(), |ui, range| {
        for f in &rows[range] {
            let _ = wip_file_row(ui, snap, f, false, open, ctx, st.session.conflict_files.count_note(t, &f.path), msgs);
        }
    });
}

fn resolved(ui: &mut egui::Ui, st: &AppState, snap: &Snapshot, lists: &Lists, open: &OpenFile, msgs: &mut Vec<Msg>) {
    let (rows, t) = (lists.resolved, st.settings.lang());
    let all = restorable_paths(st);
    let why = rows.iter().find_map(|r| blocked(st, t, r)).unwrap_or_else(|| t.tr("conflict.files.nothing_restorable.tip").into());
    let then = Msg::Conflict(ConflictMsg::MarkConflicted(all.clone()));
    let (title, button) = (t.tr("conflict.files.mark_conflicted.title"), t.tr("conflict.files.mark_conflicted.button"));
    let action = Action {
        label: t.tr("conflict.files.mark_all_conflicted.button"),
        tip: t.tr("conflict.files.mark_all_conflicted.tip"),
        why: all.is_empty().then_some(why),
        msg: Msg::confirm(title, mark_conflicted_body(t, &all), button, then),
    };
    header(ui, t, t.trf("conflict.files.resolved.header", &[("count", &rows.len())]), action, msgs);
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
    // Mark conflicted shows at the row's end while the row is hovered or open; its menu has it too.
    let at = egui::Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), super::widgets::LIST_ROW_H));
    let show_action = selected || ui.rect_contains_pointer(at);
    let end = |ui: &mut egui::Ui| {
        if !show_action {
            return;
        }
        let b = buttons::quiet(mark).small().enabled(why.is_none());
        let b = if why.is_some() { b.why(why.as_deref()) } else { b.tip(t.tr("conflict.files.mark_conflicted.tip")) };
        if ui.add(b).clicked() {
            msgs.push(confirm());
        }
    };
    let row = super::widgets::file_row_with_end(ui, selected, path, end, |ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
        paint_icon(ui.painter(), rect.center(), Icon::Check, color::added());
        super::widgets::row_text(ui, egui::RichText::new(path).monospace(), selected);
        super::path_warnings::mark(ui, t, &r.entry);
    })
    .response;
    if row.clicked() && staged {
        msgs.push(Msg::Repo(RepoMsg::OpenDiff(DiffTarget::Staged(path.clone()))));
    }
    row.row_menu(|ui| {
        super::common::menu_item_if(msgs, ui, mark, why.as_deref(), confirm());
        menu_item(msgs, ui, t.tr("common.copy_path"), Msg::Copy(path.clone()));
        if r.entry.hazard.is_none() {
            file_menu::history_items(ui, t, msgs, path, None, None);
        }
    });
}

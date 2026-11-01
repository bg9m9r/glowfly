//! Right panel for several selected commits: the list, actions on all of them, and for a pair or
//! one unbroken line of commits (from the empty tree when it starts at the first commit), the
//! comparison's direction with Swap direction, and the files that differ across them (either way
//! round) with their file actions; otherwise why there is no combined diff, and each commit's own
//! changes stacked (`details_stacked.rs`). Under the actions, why a disabled one can't run.

use super::common::short;
use super::details::file_reveal::reveal;
use super::details::{RowActions, changed_file_row_at};
use super::file_menu::RowSource;
use super::file_menu::folder_menu::{self, FolderStage};
use super::list_row::file_tree::{ListRows, Shown, folder_row, mode_toggle};
use super::menus::{cherry_pick_button, copy_shas, patch_button, revert_button};
use super::theme::color;
use super::widgets;
use super::widgets::{Icon, buttons, icon, reason};
use crate::msg::Msg;
use crate::msg::RepoMsg;
use crate::msg::SelectionMsg;
use crate::state::file_folds::FileListId;
use crate::state::lfs::TrackContext;
use crate::state::list_keys::FileList;
use crate::state::selection::MultiSelection;
use crate::state::selection_span::Span;
use crate::state::{AppState, Selection};
use crate::ui::theme::sizes::ControlSize;
use crate::ui::theme::type_scale;
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{self, Align, RichText, Sense, vec2};
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{self, DiffTarget};

pub(super) fn panel(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let (Some(snap), Selection::Commits(m)) = (&st.snap, &st.session.selection) else { return };
    let t = st.settings.lang();
    title(ui, t, m.len());
    gap_note(ui, t, m, snap);
    ui.horizontal_wrapped(|ui| {
        cherry_pick_button(ui, t, t.trn("details.multi.cherry_pick.button", m.len(), &[]), m, false, msgs);
        revert_button(ui, t, t.trn("details.multi.revert.button", m.len(), &[]), m, false, msgs);
        patch_button(ui, t, t.trn("details.multi.patch.button", m.len(), &[]), m, false, msgs);
        if ui.button(t.tr("details.multi.copy_shas.button")).clicked() {
            msgs.push(copy_shas(m));
        }
    });
    // Why a disabled action can't run, on a line of its own.
    for why in [m.pick_blocked, m.export_blocked].into_iter().flatten() {
        reason::line(ui, Some(why.text(t)), Align::Min);
    }
    ui.add_space(6.0);

    let list_h = if m.compare_ends().is_some() || m.stack_oids().is_some() { 160.0 } else { ui.available_height() };
    // Only the rows in view are drawn: a range selection can span the whole history.
    let rows: Vec<(git2::Oid, usize)> = m.oids.iter().filter_map(|&o| Some((o, *snap.row_of.get(&o)?))).collect();
    let row_h = widgets::LIST_ROW_H;
    egui::ScrollArea::vertical().id_salt("multi-commits").max_height(list_h).auto_shrink([false, true]).show_rows(
        ui,
        row_h,
        rows.len(),
        |ui, range| {
            for &(oid, row) in &rows[range] {
                let summary = &snap.commits[row].summary;
                let r = widgets::file_row(ui, false, summary, |ui| {
                    ui.label(RichText::new(short(oid)).monospace().color(color::weak()));
                    widgets::row_text(ui, RichText::new(summary), false);
                })
                .response;
                if r.clicked() {
                    msgs.push(Msg::Repo(RepoMsg::SelectCommit(oid)));
                }
            }
        },
    );

    ui.separator();
    let Some((from, to)) = m.compare_ends() else {
        super::details_stacked::show(ui, t, st, m, snap, msgs);
        return;
    };
    direction(ui, t, m, from, to, msgs);
    files(ui, t, st, m, from, to, msgs);
}

/// Why there is no combined diff, under the title: the commit whose parent isn't selected, the
/// branches the selection splits into, or the shallow clone's boundary.
fn gap_note(ui: &mut egui::Ui, t: Lang, m: &MultiSelection, snap: &glowfly_core::repo::Snapshot) {
    let note = match (m.span, m.gap()) {
        (Span::Gaps { .. }, Some(c)) => {
            let merge = snap.row_of.get(&c).is_some_and(|&r| snap.commits[r].parents.len() > 1);
            let sha = [("sha", &short(c) as &dyn std::fmt::Display)];
            if merge { t.trf("details.multi.gap_merge", &sha) } else { t.trf("details.multi.gap", &sha) }
        }
        (Span::Gaps { .. }, None) => t.tr("details.multi.gap_branches").to_owned(),
        (Span::ShallowEnd, _) => t.tr("details.multi.span_shallow").to_owned(),
        (Span::Pair { .. } | Span::Range { .. }, _) => return,
    };
    ui.add(egui::Label::new(RichText::new(note).color(color::weak())).wrap());
}

/// "3 commits selected", then " · Esc to clear" in small weak text, wrapping as one text.
fn title(ui: &mut egui::Ui, t: Lang, count: usize) {
    let mut job = LayoutJob::default();
    let strong = TextFormat { font_id: type_scale::strong(ui, type_scale::SUBHEAD), color: color::text(), ..Default::default() };
    job.append(&t.trn("details.multi.selected", count, &[]), 0.0, strong);
    let hint = TextFormat { font_id: type_scale::font(type_scale::SMALL), color: color::weak(), ..Default::default() };
    job.append(&format!(" · {}", t.tr("details.multi.esc_clear")), 0.0, hint);
    ui.add(egui::Label::new(job).wrap());
}

/// The comparison's direction: from, an arrow, to (a SHA each, or "empty repository" for the
/// empty tree a line from the first commit starts at), and Swap direction, whose tip names the
/// other way. A line of commits says how many it combines under it.
fn direction(ui: &mut egui::Ui, t: Lang, m: &MultiSelection, from: git2::Oid, to: git2::Oid, msgs: &mut Vec<Msg>) {
    let (f, o) = (short(from), short(to));
    let tip = match m.span {
        Span::Range { .. } if repo::is_empty_tree(from) => t.trf("details.multi.swap.tip_to_root", &[("from", &o)]),
        Span::Range { .. } if repo::is_empty_tree(to) => t.trf("details.multi.swap.tip_from_root", &[("to", &f)]),
        _ => t.trf("details.multi.swap.tip", &[("from", &o), ("to", &f)]),
    };
    let end = |ui: &mut egui::Ui, oid: git2::Oid| {
        if repo::is_empty_tree(oid) {
            ui.label(RichText::new(t.tr("details.multi.direction.empty")).color(color::weak()));
        } else {
            ui.label(RichText::new(short(oid)).monospace().color(color::text()));
        }
    };
    ui.horizontal_wrapped(|ui| {
        end(ui, from);
        let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
        widgets::paint_icon_sized(ui.painter(), r.center(), Icon::Into, color::weak(), icon::SIZE_SM);
        end(ui, to);
        if ui.add(buttons::icon_only(Icon::Swap, t.tr("details.multi.swap.button")).size(ControlSize::Small).tip(&tip)).clicked() {
            msgs.push(Msg::Selection(SelectionMsg::SwapCompare));
        }
    });
    if let Span::Range { .. } = m.span {
        ui.label(RichText::new(t.trn("details.multi.combined", m.len(), &[])).size(type_scale::SMALL).color(color::weak()));
    }
}

/// The files that differ from `from` to `to`, each opening its diff between them.
fn files(ui: &mut egui::Ui, t: Lang, st: &AppState, m: &MultiSelection, from: git2::Oid, to: git2::Oid, msgs: &mut Vec<Msg>) {
    match &m.changes {
        None => super::pane_status::loading_line(ui, t.tr("common.loading")),
        Some(Err(e)) => {
            let retry = Some(Msg::Selection(SelectionMsg::RetryCompare));
            super::pane_status::failed_line(
                ui,
                t,
                super::pane_status::Failure { title: t.tr("details.multi.failed.title"), error: e, retry },
                msgs,
            );
        }
        Some(Ok(c)) if c.files.is_empty() => {
            ui.label(RichText::new(t.tr("details.multi.same_files")).color(color::weak()));
        }
        Some(Ok(c)) => {
            let open = crate::state::wip_lists::cursor(st);
            let lfs = TrackContext::of(st);
            let tools = crate::state::tool_compare::ToolSetup::of(st.settings.lang(), &st.settings.external_tools);
            let commands = crate::state::custom_commands::MenuCommands::for_state(st);
            let row_h = widgets::LIST_ROW_H;
            let mode = st.settings.file_list_mode;
            let heading = t.trn("details.commit.changed_files", c.files.len(), &[]);
            super::details_wip::header::frame(ui, |ui| {
                mode_toggle(ui, t, mode, msgs);
                ui.with_layout(egui::Layout::left_to_right(Align::Center), |ui| {
                    ui.label(type_scale::section_text(ui, heading.clone()));
                });
            });
            let paths = || c.files.iter().map(|f| f.path.as_str()).collect();
            let rows = ListRows::of(ui, mode, c.files.len(), paths, st.session.tab_view.file_folds.closed(FileListId::Compare));
            let index = |t: &DiffTarget| {
                let i = c.files.iter().position(|f| *t == DiffTarget::Between(from, to, f.path.clone()))?;
                rows.row_of_file(i)
            };
            let current = open.and_then(index);
            let focus = super::file_list_keys::target(ui, FileList::Compare, rows.len(), ui.available_height(), &heading);
            let shown = egui::ScrollArea::vertical().id_salt("multi-files").auto_shrink([false, false]).show_rows(
                ui,
                row_h,
                rows.len(),
                |ui, range| {
                    reveal(ui, "multi-files", st.view.file_reveal.as_ref(), index, &range, row_h);
                    for row in range {
                        let r = match rows.get(row) {
                            Shown::File { index, depth } => {
                                let f = &c.files[index];
                                let target = DiffTarget::Between(from, to, f.path.clone());
                                let src = RowSource::Between { from, to };
                                let in_workdir = c.in_workdir(&f.path);
                                let actions = RowActions {
                                    src,
                                    history_at: m.oids[0],
                                    in_workdir,
                                    lfs,
                                    tools,
                                    patch_blocked: None,
                                    lines: None,
                                    commands: &commands,
                                };
                                changed_file_row_at(ui, t, f, (target, open), actions, depth, msgs)
                            }
                            Shown::Folder(folder) => {
                                let at = Some(m.oids[0]);
                                let menu = |ui: &mut egui::Ui, msgs: &mut Vec<Msg>, path: &str| {
                                    folder_menu::items(ui, t, msgs, path, at, FolderStage::None);
                                };
                                folder_row(ui, folder, true, msgs, FileListId::Compare, menu)
                            }
                        };
                        focus.row(ui, &r, current == Some(row));
                    }
                },
            );
            focus.rest(ui, &shown, current.is_some());
        }
    }
}

//! Right panel for one commit compared with the working directory: the commit, and the files
//! that differ from it to the working tree (staged and unstaged changes and untracked files), or
//! with Swap direction from the working tree to it, each opening that diff. The list takes the
//! keyboard focus as a whole (`file_list_keys.rs`).

use super::common::{short, with_code};
use super::details::file_reveal::reveal;
use super::details::{RowActions, changed_file_row};
use super::file_menu::RowSource;
use super::theme::color;
use crate::msg::{Msg, SelectionMsg};
use crate::state::lfs::TrackContext;
use crate::state::list_keys::FileList;
use crate::state::{AppState, Selection};
use crate::ui::theme::type_scale;
use eframe::egui::{self, RichText};
use glowfly_core::repo::DiffTarget;

pub(super) fn panel(ui: &mut egui::Ui, st: &AppState, msgs: &mut Vec<Msg>) {
    let (Some(snap), Selection::VsWorkdir(w)) = (&st.snap, &st.session.selection) else { return };
    let oid = w.oid;
    let t = st.settings.lang();
    let header = with_code(t.tr("details.workdir.header"), "{sha}", &short(oid), type_scale::SMALL, color::weak());
    ui.add(egui::Label::new(header).wrap());
    if let Some(&row) = snap.row_of.get(&oid) {
        ui.add(
            egui::Label::new(
                RichText::new(&snap.commits[row].summary).font(type_scale::strong(ui, type_scale::SUBHEAD)).color(color::text()),
            )
            .wrap(),
        );
    }
    ui.horizontal(|ui| {
        if ui.small_button(t.tr("common.copy_sha.button")).clicked() {
            msgs.push(Msg::Copy(oid.to_string()));
        }
        ui.label(RichText::new(t.tr("details.workdir.esc_hint")).size(type_scale::SMALL).color(color::weak()));
    });
    ui.separator();
    let sha = short(oid);
    // The tip names the direction the swap goes to.
    let (heading, tip) = if w.swapped {
        (t.trf("details.workdir.changes_reversed", &[("sha", &sha)]), t.trf("details.workdir.swap.tip_back", &[("sha", &sha)]))
    } else {
        (t.trf("details.workdir.changes", &[("sha", &sha)]), t.trf("details.workdir.swap.tip", &[("sha", &sha)]))
    };
    // The button goes to its own line when the heading leaves no room for it.
    ui.horizontal_wrapped(|ui| {
        ui.label(type_scale::section_text(ui, heading.clone()));
        if ui.small_button(t.tr("details.workdir.swap.button")).on_hover_text(tip).clicked() {
            msgs.push(Msg::Selection(SelectionMsg::SwapCompare));
        }
    });
    match &w.changes {
        None => super::pane_status::loading_line(ui, t.tr("common.loading")),
        Some(Err(e)) => {
            let retry = Some(Msg::Selection(SelectionMsg::RetryCompare));
            super::pane_status::failed_line(
                ui,
                t,
                super::pane_status::Failure { title: t.tr("details.workdir.failed.title"), error: e, retry },
                msgs,
            );
        }
        Some(Ok(c)) if c.files.is_empty() => {
            ui.label(RichText::new(t.tr("details.workdir.same")).color(color::weak()));
        }
        Some(Ok(c)) => {
            let open = crate::state::wip_lists::cursor(st);
            let lfs = TrackContext::of(st);
            let tools = crate::state::tool_compare::ToolSetup::of(st.settings.lang(), &st.settings.external_tools);
            let commands = crate::state::custom_commands::MenuCommands::for_state(st);
            let row_h = super::widgets::LIST_ROW_H;
            let index = |t: &DiffTarget| c.files.iter().position(|f| *t == w.target(f.path.clone()));
            let current = open.and_then(index);
            let focus = super::file_list_keys::target(ui, FileList::Workdir, c.files.len(), ui.available_height(), &heading);
            let shown = egui::ScrollArea::vertical().id_salt("workdir-files").auto_shrink([false, false]).show_rows(
                ui,
                row_h,
                c.files.len(),
                |ui, range| {
                    reveal(ui, "workdir-files", st.view.file_reveal.as_ref(), index, &range, row_h);
                    for row in range {
                        let f = &c.files[row];
                        let target = w.target(f.path.clone());
                        let src = if w.swapped { RowSource::FromWorkdir(oid) } else { RowSource::ToWorkdir(oid) };
                        let in_workdir = c.in_workdir(&f.path);
                        let actions = RowActions {
                            src,
                            history_at: oid,
                            in_workdir,
                            lfs,
                            tools,
                            patch_blocked: None,
                            lines: None,
                            commands: &commands,
                        };
                        let r = changed_file_row(ui, t, f, target, open, actions, msgs);
                        focus.row(ui, &r, current == Some(row));
                    }
                },
            );
            focus.rest(ui, &shown, current.is_some());
        }
    }
}

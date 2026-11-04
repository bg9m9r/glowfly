//! The pull request panel's changed files: the heading, notes on local branches that are
//! missing or behind, or on the head commit shown instead (with Fetch buttons; the Commits tab
//! shows the same notes), and the list, whose rows open their diff. The list takes the keyboard
//! focus as a whole (`file_list_keys.rs`).

use super::super::details::file_reveal::reveal;
use super::super::details::{RowActions, changed_file_row};
use super::super::empty_state::{EmptyIcon, empty_state};
use super::super::file_list_keys;
use super::super::file_menu::RowSource;
use super::super::pane_status::{self, Failure};
use super::super::theme::color;
use crate::theme::type_scale;
use crate::widgets::banner::{self, BannerKind};
use crate::widgets::buttons;
use eframe::egui::{self, RichText, Ui};
use glowfly_core::repo::DiffTarget;
use glowfly_model::msg::Msg;
use glowfly_model::pull_request::PullRequestMsg;
use glowfly_model::remote::RemoteMsg;
use glowfly_model::state::AppState;
use glowfly_model::state::lfs::TrackContext;
use glowfly_model::state::list_keys::FileList;
use glowfly_model::state::pull_request::{Files, PrView, empty_text, files_heading, unrelated_text};
use glowfly_model::state::pull_request_source::{
    NoteTone, fetch_blocked, head_button, head_fetch_blocked, offers_fetch, offers_head_fetch, out_of_sync, tone,
};

pub(crate) const LOADING: &str = "Comparing the branches…";
pub(crate) const FAILED_TITLE: &str = "Couldn't compare the branches";
const NO_FILES: &str = "No changed files";

pub(super) fn show(ui: &mut Ui, st: &AppState, v: &PrView, msgs: &mut Vec<Msg>) {
    let heading = match &v.files {
        Files::Loaded(c) => files_heading(c.comparison.files.len()),
        _ => "Files changed".to_owned(),
    };
    ui.label(type_scale::section_text(ui, heading.clone()));
    notes(ui, st, v, msgs);
    let weak = |ui: &mut Ui, text: String| ui.add(egui::Label::new(RichText::new(text).color(color::weak())).wrap());
    match &v.files {
        Files::Missing => {}
        Files::Loading => pane_status::loading(ui, LOADING),
        Files::Unrelated => {
            let shallow = st.snap.as_ref().is_some_and(|s| !s.shallow_roots.is_empty());
            weak(ui, unrelated_text(v, shallow));
        }
        Files::Failed(e) => {
            let retry = Some(Msg::PullRequest(PullRequestMsg::ReloadFiles));
            pane_status::failed(ui, st.settings.lang(), Failure { title: FAILED_TITLE, error: e, retry }, msgs);
        }
        Files::Loaded(c) if c.comparison.files.is_empty() => {
            empty_state(ui, EmptyIcon::File, NO_FILES, &empty_text(v), &[]);
        }
        Files::Loaded(c) => {
            let Some(tip) = v.source_tip() else { return };
            let (base, files) = (c.base, &c.comparison.files);
            let open = glowfly_model::state::wip_lists::cursor(st);
            let lfs = TrackContext::of(st);
            let tools = glowfly_model::state::tool_compare::ToolSetup::of(st.settings.lang(), &st.settings.external_tools);
            let commands = glowfly_model::state::custom_commands::MenuCommands::for_state(st);
            let row_h = crate::widgets::LIST_ROW_H;
            let index = |t: &DiffTarget| files.iter().position(|f| v.diff_target(&f.path).as_ref() == Some(t));
            let current = open.and_then(index);
            let focus = file_list_keys::target(ui, FileList::PullRequest, files.len(), ui.available_height(), &heading);
            let area = egui::ScrollArea::vertical().id_salt("pr-files").auto_shrink([false, false]);
            let shown = area.show_rows(ui, row_h, files.len(), |ui, range| {
                reveal(ui, "pr-files", st.cues.file_reveal.as_ref(), index, &range, row_h);
                for row in range {
                    let f = &files[row];
                    let Some(target) = v.diff_target(&f.path) else { continue };
                    let src = RowSource::Between { from: base, to: tip };
                    let in_workdir = c.comparison.in_workdir(&f.path);
                    let actions =
                        RowActions { src, history_at: tip, in_workdir, lfs, tools, patch_blocked: None, lines: None, commands: &commands };
                    let r = changed_file_row(ui, st.settings.lang(), f, target, open, actions, msgs);
                    focus.row(ui, &r, current == Some(row));
                }
            });
            focus.rest(ui, &shown, current.is_some());
        }
    }
}

/// One note per local branch that is missing or behind (or on the head shown instead), with
/// "Fetch <remote>" and, when the service keeps a head ref, "Fetch #7's head".
pub(super) fn notes(ui: &mut Ui, st: &AppState, v: &PrView, msgs: &mut Vec<Msg>) {
    let notes = out_of_sync(v);
    let Some(snap) = st.snap.as_ref().filter(|_| !notes.is_empty()) else { return };
    let kind = match tone(&notes) {
        NoteTone::Info => BannerKind::Info,
        NoteTone::Progress => BannerKind::Progress,
        NoteTone::Warning => BannerKind::Warning,
    };
    banner::frame(ui, kind, None, |ui| {
        for n in &notes {
            ui.add(egui::Label::new(RichText::new(n.text(v)).color(color::text())).wrap());
        }
        let (fetch, head) = (offers_fetch(&notes), offers_head_fetch(v, &notes));
        if !fetch && !head {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            if fetch {
                let blocked = fetch_blocked(snap, &v.remote, st.job.is_none());
                let r = ui.add(buttons::secondary(format!("Fetch {}", v.remote)).enabled(blocked.is_none()).why(blocked.as_deref()));
                if r.clicked() {
                    msgs.push(Msg::Remote(RemoteMsg::Fetch(v.remote.clone())));
                }
            }
            if head {
                let blocked = head_fetch_blocked(snap, v, st.runs_git_unasked());
                let r = ui.add(buttons::secondary(head_button(v)).enabled(blocked.is_none()).why(blocked.as_deref()));
                if r.clicked() {
                    msgs.push(Msg::PullRequest(PullRequestMsg::FetchHead));
                }
            }
        });
    });
    ui.add_space(4.0);
}

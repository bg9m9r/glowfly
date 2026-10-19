//! The pull request panel's changed files: the heading, notes on local branches that are
//! missing or behind (with a Fetch button), and the list, whose rows open their diff.

use super::super::details::file_reveal::reveal;
use super::super::details::{RowActions, changed_file_row};
use super::super::empty_state::{EmptyIcon, empty_state};
use super::super::file_menu::RowSource;
use super::super::pane_status::{self, Failure};
use super::super::theme::color;
use crate::msg::Msg;
use crate::pull_request::PullRequestMsg;
use crate::remote::RemoteMsg;
use crate::state::AppState;
use crate::state::lfs::TrackContext;
use crate::state::pull_request::{Files, PrView, empty_text, fetch_blocked, files_heading, out_of_sync, unrelated_text};
use crate::ui::theme::type_scale;
use crate::ui::widgets::banner::{self, BannerKind};
use crate::ui::widgets::buttons;
use eframe::egui::{self, RichText, Ui};
use trellis_core::repo::DiffTarget;

pub(in crate::ui) const LOADING: &str = "Comparing the branches…";
pub(in crate::ui) const FAILED_TITLE: &str = "Couldn't compare the branches";
const NO_FILES: &str = "No changed files";

pub(super) fn show(ui: &mut Ui, st: &AppState, v: &PrView, msgs: &mut Vec<Msg>) {
    let heading = match &v.files {
        Files::Loaded(c) => files_heading(c.comparison.files.len()),
        _ => "Files changed".to_owned(),
    };
    ui.label(type_scale::section_text(ui, heading));
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
            let Some(tip) = v.tips.source else { return };
            let (base, files) = (c.base, &c.comparison.files);
            let open = st.diff.as_ref().map(|(t, _)| t);
            let lfs = TrackContext::of(st);
            let diff_blocked = crate::state::tool_compare::diff_blocked(&st.settings.external_tools);
            let row_h = crate::ui::widgets::LIST_ROW_H;
            egui::ScrollArea::vertical().id_salt("pr-files").auto_shrink([false, false]).show_rows(ui, row_h, files.len(), |ui, range| {
                let index = |t: &DiffTarget| files.iter().position(|f| v.diff_target(&f.path).as_ref() == Some(t));
                reveal(ui, "pr-files", st.view.file_reveal.as_ref(), index, &range, row_h);
                for f in &files[range] {
                    let Some(target) = v.diff_target(&f.path) else { continue };
                    let src = RowSource::Between { from: base, to: tip };
                    let in_workdir = c.comparison.in_workdir(&f.path);
                    let actions = RowActions { src, history_at: tip, in_workdir, lfs, diff_blocked, patch_blocked: None, lines: None };
                    changed_file_row(ui, st.settings.lang(), f, target, open, actions, msgs);
                }
            });
        }
    }
}

/// One note per local branch that is missing or behind, and a Fetch button.
fn notes(ui: &mut Ui, st: &AppState, v: &PrView, msgs: &mut Vec<Msg>) {
    let notes = out_of_sync(v);
    let Some(snap) = st.snap.as_ref().filter(|_| !notes.is_empty()) else { return };
    banner::frame(ui, BannerKind::Warning, None, |ui| {
        for n in &notes {
            ui.add(egui::Label::new(RichText::new(n.text(v)).color(color::text())).wrap());
        }
        let blocked = fetch_blocked(snap, &v.remote, st.job.is_none());
        let mut r = ui.add(buttons::secondary(format!("Fetch {}", v.remote)).enabled(blocked.is_none()));
        if let Some(why) = &blocked {
            r = r.on_hover_text(why);
        }
        if r.clicked() {
            msgs.push(Msg::Remote(RemoteMsg::Fetch(v.remote.clone())));
        }
    });
    ui.add_space(4.0);
}

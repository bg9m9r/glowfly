//! The pull request panel's changed files: the heading, notes on local branches that are
//! missing or behind (with a Fetch button), and the list, whose rows open their diff.

use super::super::common::widget_row_height;
use super::super::details::changed_file_row;
use super::super::file_menu::RowSource;
use super::super::theme::color;
use crate::msg::Msg;
use crate::remote::RemoteMsg;
use crate::state::AppState;
use crate::state::lfs::TrackContext;
use crate::state::pull_request::{Files, PrView, empty_text, fetch_blocked, files_heading, out_of_sync, unrelated_text};
use eframe::egui::{self, RichText, Ui};

pub(super) fn show(ui: &mut Ui, st: &AppState, v: &PrView, msgs: &mut Vec<Msg>) {
    let heading = match &v.files {
        Files::Loaded(c) => files_heading(c.comparison.files.len()),
        _ => "Files changed".to_owned(),
    };
    ui.label(RichText::new(heading).strong().color(color::text()));
    notes(ui, st, v, msgs);
    let weak = |ui: &mut Ui, text: String| ui.add(egui::Label::new(RichText::new(text).color(color::weak())).wrap());
    match &v.files {
        Files::Missing => {}
        Files::Loading => {
            weak(ui, "Loading…".to_owned());
        }
        Files::Unrelated => {
            let shallow = st.snap.as_ref().is_some_and(|s| !s.shallow_roots.is_empty());
            weak(ui, unrelated_text(v, shallow));
        }
        Files::Failed(e) => {
            ui.add(egui::Label::new(RichText::new(e).color(color::red())).wrap());
        }
        Files::Loaded(c) if c.comparison.files.is_empty() => {
            weak(ui, empty_text(v));
        }
        Files::Loaded(c) => {
            let Some(tip) = v.tips.source else { return };
            let (base, files) = (c.base, &c.comparison.files);
            let open = st.diff.as_ref().map(|(t, _)| t);
            let lfs = TrackContext::of(st);
            egui::ScrollArea::vertical().id_salt("pr-files").auto_shrink([false, false]).show_rows(
                ui,
                widget_row_height(ui),
                files.len(),
                |ui, range| {
                    for f in &files[range] {
                        let Some(target) = v.diff_target(&f.path) else { continue };
                        let source = RowSource::Between { older: base, newer: tip };
                        changed_file_row(ui, f, target, open, tip, Some((source, c.comparison.in_workdir(&f.path), lfs)), msgs);
                    }
                },
            );
        }
    }
}

/// One note per local branch that is missing or behind, and a Fetch button.
fn notes(ui: &mut Ui, st: &AppState, v: &PrView, msgs: &mut Vec<Msg>) {
    let notes = out_of_sync(v);
    let Some(snap) = st.snap.as_ref().filter(|_| !notes.is_empty()) else { return };
    egui::Frame::group(ui.style()).fill(color::banner()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        for n in &notes {
            ui.add(egui::Label::new(RichText::new(n.text(v)).color(color::text())).wrap());
        }
        let blocked = fetch_blocked(snap, &v.remote, st.job.is_none());
        let mut r = ui.add_enabled(blocked.is_none(), egui::Button::new(format!("Fetch {}", v.remote)));
        if let Some(why) = &blocked {
            r = r.on_disabled_hover_text(why);
        }
        if r.clicked() {
            msgs.push(Msg::Remote(RemoteMsg::Fetch(v.remote.clone())));
        }
    });
    ui.add_space(4.0);
}

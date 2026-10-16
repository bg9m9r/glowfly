//! Preferences > General's Updates row: the daily check, this version and when it last checked,
//! Check now, and the check's or download's status with what can be done next.

use super::{help, row};
use crate::msg::{Msg, UpdateMsg};
use crate::state::updates::{CHECKING, DOWNLOADING, UP_TO_DATE, UpdateStatus, UpdatesState, available_text, download_failed_text};
use crate::ui::theme::color;
use eframe::egui::{self, RichText, Ui};
use trellis_core::settings::Settings;

/// What the row shows, gathered before the settings are borrowed for editing.
pub struct UpdatesView<'a> {
    pub updates: &'a UpdatesState,
    /// The last successful check, formatted with the date settings.
    pub last_checked: Option<String>,
}

pub fn updates_row(ui: &mut Ui, s: &mut Settings, view: &UpdatesView, msgs: &mut Vec<Msg>) {
    let u = view.updates;
    let send = |msgs: &mut Vec<Msg>, m: UpdateMsg| msgs.push(Msg::Updates(m));
    row(ui, "Updates", |ui| {
        ui.checkbox(&mut s.updates.check, "Check for updates once a day");
        ui.label(RichText::new(u.prefs_line(view.last_checked.as_deref())).color(color::text()));
        if ui.add_enabled(!u.busy(), egui::Button::new("Check now")).clicked() {
            send(msgs, UpdateMsg::CheckNow);
        }
        match &u.status {
            UpdateStatus::Idle => {}
            UpdateStatus::Checking { .. } => spinning(ui, CHECKING),
            UpdateStatus::UpToDate => {
                ui.label(RichText::new(UP_TO_DATE).color(color::green()));
            }
            UpdateStatus::CheckFailed(e) => {
                ui.add(egui::Label::new(RichText::new(format!("Couldn't check: {e}")).color(color::red())).wrap());
                if ui.button("Try again").clicked() {
                    send(msgs, UpdateMsg::CheckNow);
                }
            }
            UpdateStatus::Available(o) => {
                ui.label(RichText::new(available_text(&o.version)).strong().color(color::blue()));
                ui.horizontal(|ui| {
                    let label = if o.asset.is_some() { "Download" } else { "Open release page" };
                    if ui.button(label).clicked() {
                        send(msgs, if o.asset.is_some() { UpdateMsg::Download } else { UpdateMsg::OpenReleasePage });
                    }
                    if ui.button("Release notes").clicked() {
                        send(msgs, UpdateMsg::ShowNotes);
                    }
                });
            }
            UpdateStatus::Downloading(_) => spinning(ui, DOWNLOADING),
            UpdateStatus::Downloaded { file, installer, .. } => {
                let shown = trellis_core::native_path::shown(file);
                ui.add(egui::Label::new(RichText::new(format!("Downloaded to {shown}")).color(color::text())).wrap());
                let (label, m) = if *installer { ("Install", UpdateMsg::Install) } else { ("Show file", UpdateMsg::ShowFile) };
                if ui.button(label).clicked() {
                    send(msgs, m);
                }
            }
            UpdateStatus::DownloadFailed { error, .. } => {
                ui.add(egui::Label::new(RichText::new(download_failed_text(error)).color(color::red())).wrap());
                if ui.button("Try again").clicked() {
                    send(msgs, UpdateMsg::Download);
                }
            }
        }
        help(
            ui,
            "Trellis asks GitHub's releases API for the newest release of its repository once a day. Nothing about you or your \
             repositories is sent. Downloads are checked against the release's published SHA-256 checksums.",
        );
    });
}

fn spinning(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add(egui::Spinner::new().size(12.0));
        ui.label(RichText::new(text).color(color::weak()));
    });
}

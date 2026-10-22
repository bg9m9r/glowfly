//! Preferences > General's Updates row: the daily check, this version and when it last checked,
//! Check now, and the check's or download's status with what can be done next (Download, Install
//! and restart, Show file).

use super::{help, row};
use crate::msg::{Msg, UpdateMsg};
use crate::state::updates::{UpdateStatus, UpdatesState, available_text, download_failed_text, install_label};
use crate::ui::theme::color;
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{self, RichText, Ui};
use glowfly_core::app_update::InstallMethod;
use glowfly_core::i18n::Lang;
use glowfly_core::settings::Settings;
use std::path::Path;

/// What the row shows, gathered before the settings are borrowed for editing.
pub struct UpdatesView<'a> {
    pub updates: &'a UpdatesState,
    /// The last successful check, formatted with the date settings.
    pub last_checked: Option<String>,
}

pub fn updates_row(ui: &mut Ui, s: &mut Settings, view: &UpdatesView, msgs: &mut Vec<Msg>) {
    let u = view.updates;
    let send = |msgs: &mut Vec<Msg>, m: UpdateMsg| msgs.push(Msg::Updates(m));
    let t = s.lang();
    row(ui, t.tr("prefs.general.updates.label"), |ui| {
        controls::checkbox(ui, &mut s.updates.check, t.tr("prefs.general.updates.check"));
        ui.label(RichText::new(u.prefs_line(t, view.last_checked.as_deref())).color(color::text()));
        if ui.add(buttons::secondary(t.tr("prefs.general.updates.check_now.button")).enabled(!u.blocked())).clicked() {
            send(msgs, UpdateMsg::CheckNow);
        }
        match &u.status {
            UpdateStatus::Idle => {}
            UpdateStatus::Checking { .. } => spinning(ui, t.tr("updates.checking")),
            UpdateStatus::UpToDate => {
                ui.label(RichText::new(t.tr("updates.up_to_date")).color(color::added()));
            }
            UpdateStatus::CheckFailed(e) => {
                let text = t.trf("prefs.general.updates.couldnt_check", &[("error", e)]);
                ui.add(egui::Label::new(RichText::new(text).color(color::removed())).wrap());
                if ui.add(buttons::secondary(t.tr("updates.try_again.button"))).clicked() {
                    send(msgs, UpdateMsg::CheckNow);
                }
            }
            UpdateStatus::Available(o) => {
                ui.label(RichText::new(available_text(t, &o.version)).font(type_scale::strong(ui, type_scale::BODY)).color(color::info()));
                ui.horizontal(|ui| {
                    let label = if o.asset.is_some() { t.tr("updates.download.button") } else { t.tr("updates.open_release_page.button") };
                    if ui.add(buttons::secondary(label)).clicked() {
                        send(msgs, if o.asset.is_some() { UpdateMsg::Download } else { UpdateMsg::OpenReleasePage });
                    }
                    if ui.add(buttons::secondary(t.tr("updates.release_notes.button"))).clicked() {
                        send(msgs, UpdateMsg::ShowNotes);
                    }
                });
            }
            UpdateStatus::Downloading(_) => spinning(ui, t.tr("updates.downloading")),
            UpdateStatus::Downloaded { file, method, .. } => downloaded_row(ui, t, u, file, *method, msgs),
            UpdateStatus::DownloadFailed { error, .. } => {
                ui.add(egui::Label::new(RichText::new(download_failed_text(t, error)).color(color::removed())).wrap());
                if ui.add(buttons::secondary(t.tr("updates.try_again.button"))).clicked() {
                    send(msgs, UpdateMsg::Download);
                }
            }
        }
        help(ui, t.tr("prefs.general.updates.help"));
    });
}

/// A downloaded update: where it is, Install and restart (or Open installer and quit) when it
/// can be installed, Show file, and what installing it does when Glowfly doesn't restart.
fn downloaded_row(ui: &mut Ui, t: Lang, u: &UpdatesState, file: &Path, method: InstallMethod, msgs: &mut Vec<Msg>) {
    if u.restarting {
        spinning(ui, t.tr("updates.preparing"));
        return;
    }
    let shown = glowfly_core::native_path::shown(file);
    let text = t.trf("prefs.general.updates.downloaded_to", &[("path", &shown)]);
    ui.add(egui::Label::new(RichText::new(text).color(color::text())).wrap());
    ui.horizontal(|ui| {
        if method.acts() && ui.add(buttons::secondary(install_label(t, method))).clicked() {
            msgs.push(Msg::Updates(UpdateMsg::InstallAndRestart));
        }
        if ui.add(buttons::secondary(t.tr("updates.show_file.button"))).clicked() {
            msgs.push(Msg::Updates(UpdateMsg::ShowFile));
        }
    });
    let ext = file.extension().map(|x| x.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let note = match method {
        InstallMethod::OpenAndQuit if ext == "dmg" => t.tr("prefs.general.updates.open_quit_dmg.help"),
        InstallMethod::OpenAndQuit if ext == "deb" => t.tr("prefs.general.updates.open_quit_deb.help"),
        InstallMethod::OpenAndQuit => t.tr("prefs.general.updates.open_quit_msi.help"),
        InstallMethod::ShowOnly => t.tr("prefs.general.updates.show_only.help"),
        InstallMethod::RunMsi | InstallMethod::SwapPortable => return,
    };
    help(ui, note);
}

fn spinning(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add(egui::Spinner::new().size(12.0));
        ui.label(RichText::new(text).color(color::weak()));
    });
}

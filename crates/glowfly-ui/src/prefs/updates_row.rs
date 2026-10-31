//! Preferences > General's Updates row: the daily check, this version and when it last checked,
//! Check now, and the check's or download's status with what can be done next (Download, its
//! progress and Cancel, Install and restart, Show file, and Download again when the downloaded
//! file is gone or changed). A Flatpak copy's offer says how Flatpak updates it instead of
//! offering a download.

use super::{help, row, warn};
use crate::msg::{Msg, UpdateMsg};
use crate::state::updates::{
    Transfer, UpdateStatus, UpdatesState, available_text, download_failed_text, install_label, progress_text, stale_text,
};
use crate::ui::theme::color;
use crate::ui::theme::controls;
use crate::ui::theme::tokens::radius;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{self, CornerRadius, Rect, RichText, Sense, Ui, WidgetInfo, WidgetType, vec2};
use glowfly_core::app_update::Offer;
use glowfly_core::app_update::{InstallMethod, SavedProblem};
use glowfly_core::i18n::Lang;
use glowfly_core::settings::Settings;
use std::path::Path;

/// The progress bar's widest and its height.
const BAR_W: f32 = 320.0;
const BAR_H: f32 = 6.0;

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
            UpdateStatus::Available(o) if o.through_flatpak() => flatpak_available(ui, t, o, msgs),
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
            UpdateStatus::Downloading(_) => downloading(ui, t, u.transfer, msgs),
            UpdateStatus::Downloaded { file, method, .. } => downloaded_row(ui, t, u, file, *method, msgs),
            UpdateStatus::DownloadFailed { error, .. } => {
                ui.add(egui::Label::new(RichText::new(download_failed_text(t, error)).color(color::removed())).wrap());
                if ui.add(buttons::secondary(t.tr("updates.try_again.button"))).clicked() {
                    send(msgs, UpdateMsg::Download);
                }
            }
            UpdateStatus::Stale { offer, file, problem } => stale(ui, t, &offer.version, file, *problem, msgs),
        }
        help(ui, t.tr("prefs.general.updates.help"));
    });
}

/// A newer release for a Flatpak copy: how Flatpak updates it, Release notes and Open release
/// page; nothing to download.
fn flatpak_available(ui: &mut Ui, t: Lang, o: &Offer, msgs: &mut Vec<Msg>) {
    ui.label(RichText::new(available_text(t, &o.version)).font(type_scale::strong(ui, type_scale::BODY)).color(color::info()));
    ui.add(egui::Label::new(RichText::new(t.tr("updates.flatpak.how")).color(color::text())).wrap());
    ui.horizontal(|ui| {
        if ui.add(buttons::secondary(t.tr("updates.release_notes.button"))).clicked() {
            msgs.push(Msg::Updates(UpdateMsg::ShowNotes));
        }
        if ui.add(buttons::secondary(t.tr("updates.open_release_page.button"))).clicked() {
            msgs.push(Msg::Updates(UpdateMsg::OpenReleasePage));
        }
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
        InstallMethod::OpenAndQuit if ext == "deb" || ext == "rpm" => t.tr("prefs.general.updates.open_quit_deb.help"),
        InstallMethod::OpenAndQuit => t.tr("prefs.general.updates.open_quit_msi.help"),
        InstallMethod::ShowOnly => t.tr("prefs.general.updates.show_only.help"),
        InstallMethod::RunMsi | InstallMethod::SwapPortable | InstallMethod::SwapAppImage => return,
    };
    help(ui, note);
}

/// The running download: how far it is (a bar when its size is known) and Cancel, or
/// "Cancelling…" once Cancel was pressed.
fn downloading(ui: &mut Ui, t: Lang, x: Transfer, msgs: &mut Vec<Msg>) {
    if x.cancelling {
        spinning(ui, t.tr("updates.cancelling"));
        return;
    }
    let text = progress_text(t, x.progress);
    match x.progress.fraction() {
        Some(f) => {
            ui.label(RichText::new(&text).color(color::text()));
            bar(ui, f);
        }
        None => spinning(ui, &text),
    }
    if ui.add(buttons::secondary(t.tr("common.cancel.button"))).clicked() {
        msgs.push(Msg::Updates(UpdateMsg::CancelDownload));
    }
}

/// A thin accent bar `fraction` full, read out as a progress indicator.
fn bar(ui: &mut Ui, fraction: f32) {
    let w = ui.available_width().min(BAR_W);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, BAR_H), Sense::hover());
    let r = CornerRadius::same(radius::XS);
    ui.painter().rect_filled(rect, r, color::border());
    let done = Rect::from_min_size(rect.min, vec2(w * fraction.clamp(0.0, 1.0), BAR_H));
    ui.painter().rect_filled(done, r, color::accent());
    resp.widget_info(|| {
        let mut info = WidgetInfo::new(WidgetType::ProgressIndicator);
        info.value = Some(f64::from((fraction * 100.0).floor()));
        info
    });
}

/// The file downloaded for `version` is gone or changed: why, Download again and Release notes.
fn stale(ui: &mut Ui, t: Lang, version: &glowfly_core::app_update::Version, file: &Path, problem: SavedProblem, msgs: &mut Vec<Msg>) {
    ui.label(RichText::new(available_text(t, version)).font(type_scale::strong(ui, type_scale::BODY)).color(color::info()));
    warn(ui, &stale_text(t, file, problem));
    ui.horizontal(|ui| {
        if ui.add(buttons::secondary(t.tr("updates.download_again.button"))).clicked() {
            msgs.push(Msg::Updates(UpdateMsg::Download));
        }
        if ui.add(buttons::secondary(t.tr("updates.release_notes.button"))).clicked() {
            msgs.push(Msg::Updates(UpdateMsg::ShowNotes));
        }
    });
}

fn spinning(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add(egui::Spinner::new().size(12.0));
        ui.label(RichText::new(text).color(color::weak()));
    });
}

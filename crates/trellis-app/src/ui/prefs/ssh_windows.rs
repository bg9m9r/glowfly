//! Preferences > SSH, in the Agent section: the Windows' OpenSSH row. When git runs Git for
//! Windows' bundled ssh, which doesn't read keys from the agent service, it offers Use Windows'
//! OpenSSH (global core.sshCommand); once that is set, Use Git's own ssh undoes it. Hidden on
//! other systems, and whenever `state::ssh_windows::windows_offer` says so. Every action is a
//! `Msg`.

use super::{help, warn};
use crate::msg::Msg;
use crate::ssh::SshMsg;
use crate::state::ssh::SshPage;
use crate::state::ssh_windows::{WindowsOffer, windows_blocked, windows_offer};
use crate::ui::widgets::buttons;
use eframe::egui::{self, Ui};
use trellis_core::i18n::Lang;
use trellis_core::native_path;

pub(super) fn windows_row(ui: &mut Ui, t: Lang, page: &SshPage, msgs: &mut Vec<Msg>) {
    let offer = windows_offer(page, t);
    if offer != WindowsOffer::Hidden {
        ui.add_space(4.0);
    }
    match offer {
        WindowsOffer::Hidden => {}
        WindowsOffer::Offer { windows, bundled, old } => {
            help(ui, &t.trf("prefs.ssh.windows.bundled.help", &[("path", &native_path::shown(&bundled))]));
            let tip = t.trf("prefs.ssh.windows.use.tip", &[("path", &native_path::shown(&windows))]);
            button(ui, t, page, t.tr("prefs.ssh.windows.use.button"), &tip, true, msgs);
            old_warn(ui, t, old);
        }
        WindowsOffer::Blocked { bundled, why, .. } => {
            help(ui, &t.trf("prefs.ssh.windows.bundled.help", &[("path", &native_path::shown(&bundled))]));
            let use_button = buttons::secondary(t.tr("prefs.ssh.windows.use.button")).enabled(false).why(Some(why));
            ui.add(use_button);
            warn(ui, why);
        }
        WindowsOffer::On { windows, service_running, old } => {
            help(ui, &t.trf("prefs.ssh.windows.on.help", &[("path", &native_path::shown(&windows))]));
            if !service_running {
                help(ui, t.tr("prefs.ssh.windows.service_off.help"));
            }
            button(ui, t, page, t.tr("prefs.ssh.windows.undo.button"), t.tr("prefs.ssh.windows.undo.tip"), false, msgs);
            old_warn(ui, t, old);
        }
        WindowsOffer::Already => help(ui, t.tr("prefs.ssh.windows.already.help")),
    }
    if let Some(e) = &page.windows_error {
        warn(ui, e);
    }
}

/// The switch (`on`) or undo button, with a spinner while a global core.sshCommand write runs.
fn button(ui: &mut Ui, t: Lang, page: &SshPage, label: &str, tip: &str, on: bool, msgs: &mut Vec<Msg>) {
    ui.horizontal(|ui| {
        let blocked = windows_blocked(page, t);
        let b = buttons::secondary(label).enabled(blocked.is_none()).why(blocked).tip(tip);
        if ui.add(b).clicked() {
            msgs.push(Msg::Ssh(SshMsg::UseWindowsSsh(on)));
        }
        if page.choice_busy && !page.choice_repo {
            ui.add(egui::Spinner::new().size(14.0));
        }
    });
}

/// The warning for a Windows' ssh older than 8.4.
fn old_warn(ui: &mut Ui, t: Lang, old: Option<(u32, u32)>) {
    if let Some((major, minor)) = old {
        warn(ui, &t.trf("prefs.ssh.windows.old.warn", &[("version", &format!("{major}.{minor}"))]));
    }
}

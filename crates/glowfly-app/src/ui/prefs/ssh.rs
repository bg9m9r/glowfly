//! Preferences > SSH: one column of three sections. Keys lists the key pairs as cards
//! (`ssh_card`, with the Add to service form under its card, `ssh_service`), the ~/.ssh/config
//! entries that can't be listed and the keys only the agent holds (`ssh_sources`), and offers
//! Generate… (the Generate SSH key dialog, `ui/ssh_keygen_dialog.rs`) and Use existing…; Which
//! key picks the key git uses (`ssh_git`); Agent shows the local SSH agent (`ssh_agent`, with
//! the Windows' OpenSSH row from `ssh_windows`). The
//! view edits only text buffers; everything else is a `Msg`.

use super::{GAP, LABEL_W, ROW_W, fit, help, warn};
use crate::msg::Msg;
use crate::ssh::SshMsg;
use crate::state::ssh::SshPage;
use crate::ui::theme::color;
use crate::ui::widgets::buttons;
use eframe::egui::{self, RichText, Ui};
use glowfly_core::forge::Account;
use glowfly_core::i18n::Lang;
use glowfly_core::native_path;
use glowfly_core::paths::same_spelling;
use glowfly_core::ssh_keys::{KeyFileProblem, SshListing};
use std::path::PathBuf;

/// What the page needs from outside Preferences.
pub struct SshContext {
    /// Settings > Use local SSH agent.
    pub use_agent: bool,
    /// Connected code-hosting accounts, with their menu labels.
    pub services: Vec<(Account, String)>,
    /// The running git job's label.
    pub job: Option<String>,
    /// The keys added with Use existing (`Settings::ssh_extra_keys`): only those can be removed
    /// from the list.
    pub extra: Vec<PathBuf>,
}

pub fn ssh_page(ui: &mut Ui, t: Lang, page: &mut SshPage, cx: &SshContext, msgs: &mut Vec<Msg>) {
    let reload = Msg::Ssh(SshMsg::Reload);
    super::ssh_agent::header(ui, t, t.tr("prefs.ssh.keys.title"), None, t.tr("prefs.ssh.keys.reload.tip"), reload, msgs);
    if let Some(error) = &page.error {
        ui.label(RichText::new(t.tr("prefs.ssh.keys.error.label")).color(color::text()));
        warn(ui, error);
        ui.add_space(4.0);
        if ui.add(buttons::secondary(t.tr("prefs.ssh.keys.reload.button"))).clicked() {
            msgs.push(Msg::Ssh(SshMsg::Reload));
        }
        return;
    }
    // The rest of the page waits for the listing.
    let Some(listing) = page.listing.clone() else {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new());
            ui.label(RichText::new(t.tr("prefs.ssh.keys.reading")).color(color::weak()));
        });
        return;
    };
    keys_section(ui, t, page, &listing, cx, msgs);
    ui.add_space(14.0);
    super::ssh_git::which_key(ui, t, page, &listing, cx, msgs);
    super::ssh_agent::agent_section(ui, t, page, cx, msgs);
}

fn keys_section(ui: &mut Ui, t: Lang, page: &mut SshPage, listing: &SshListing, cx: &SshContext, msgs: &mut Vec<Msg>) {
    let dir = native_path::shown(&listing.dir);
    ui.vertical(|ui| {
        ui.set_max_width(fit(ui, LABEL_W + GAP + ROW_W));
        let config = native_path::shown(&listing.config_file);
        help(ui, &t.trf("prefs.ssh.keys.help", &[("dir", &dir), ("config", &config)]));
        ui.add_space(6.0);
        if listing.keygen.is_none() {
            warn(ui, t.tr("prefs.ssh.keys.no_keygen.warn"));
            ui.add_space(4.0);
        }
        if listing.keys.is_empty() {
            help(ui, &t.trf("prefs.ssh.keys.empty", &[("dir", &dir)]));
            ui.add_space(6.0);
        }
    });
    for key in &listing.keys {
        super::ssh_card::card(ui, t, key, listing, page, cx, msgs);
        if let Some(form) = page.upload.as_mut().filter(|f| same_spelling(&f.key, &key.private)) {
            super::ssh_service::upload_form(ui, t, form, &key.file_name(), msgs);
        }
    }
    super::ssh_sources::config_notes(ui, t, listing);
    super::ssh_sources::agent_only_rows(ui, t, page, msgs);
    ui.horizontal(|ui| {
        let blocked = page.open_blocked(t);
        let generate = buttons::secondary(t.tr("prefs.ssh.keys.generate.button")).enabled(blocked.is_none()).why(blocked);
        if ui.add(generate).clicked() {
            msgs.push(Msg::Ssh(SshMsg::OpenGenerate));
        }
        let existing = buttons::secondary(t.tr("prefs.ssh.keys.use_existing.button")).tip(t.tr("prefs.ssh.keys.use_existing.tip"));
        if ui.add(existing).clicked() {
            msgs.push(Msg::Ssh(SshMsg::UseExisting));
        }
    });
    if let Some((file, problem)) = &page.pick_error {
        let file: &dyn std::fmt::Display = file;
        let text = match problem {
            KeyFileProblem::NoPublicKey => t.trf("prefs.ssh.keys.pick.no_public.warn", &[("file", file)]),
            KeyFileProblem::NotAPublicKey => t.trf("prefs.ssh.keys.pick.not_a_key.warn", &[("file", file)]),
            KeyFileProblem::NotAFile => t.trf("prefs.ssh.keys.pick.not_a_file.warn", &[("file", file)]),
            KeyFileProblem::Unreadable(e) => t.trf("prefs.ssh.keys.pick.failed.warn", &[("error", e)]),
        };
        ui.add_space(4.0);
        ui.vertical(|ui| {
            ui.set_max_width(fit(ui, LABEL_W + GAP + ROW_W));
            warn(ui, &text);
        });
    }
}

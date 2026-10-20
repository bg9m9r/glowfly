//! Preferences > SSH: the Which key section. One row per level (all repositories, and the open
//! repository), each a dropdown of the listed keys plus Let ssh decide, showing what
//! core.sshCommand says there. A pick sends the existing `SshMsg::UseKey` / `SshMsg::StopUsing`
//! (unconfirmed, so a command Trellis didn't write still asks first); the view never writes
//! config itself.

use super::ssh::SshContext;
use super::{fit, help, row, warn};
use crate::msg::Msg;
use crate::ssh::SshMsg;
use crate::state::ssh::SshPage;
use crate::state::ssh_use::use_blocked;
use crate::state::trust::shown_value;
use crate::ui::theme::{color, type_scale};
use eframe::egui::{self, RichText, Ui, WidgetInfo, WidgetType};
use trellis_core::i18n::Lang;
use trellis_core::native_path;
use trellis_core::paths::same_spelling;
use trellis_core::ssh_keys::{KeyChoice, SshListing};

pub(super) fn which_key(ui: &mut Ui, t: Lang, page: &SshPage, listing: &SshListing, cx: &SshContext, msgs: &mut Vec<Msg>) {
    super::section(ui, t.tr("prefs.ssh.which.title"));
    ui.vertical(|ui| {
        ui.set_max_width(fit(ui, super::LABEL_W + super::GAP + super::ROW_W));
        help(ui, t.tr("prefs.ssh.which.help"));
    });
    ui.add_space(10.0);
    let Some(choices) = &page.choice else {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new().size(14.0));
            ui.label(RichText::new(t.tr("prefs.ssh.which.reading")).size(type_scale::SMALL).color(color::weak()));
        });
        ui.add_space(18.0);
        return;
    };
    let level = Level { t, page, listing, cx };
    level.row(ui, t.tr("prefs.ssh.which.all.label"), false, &choices.global, msgs);
    if let (Some(repo), Some(choice)) = (&page.repo, &choices.repo) {
        level.row(ui, &t.trf("prefs.ssh.which.repo.label", &[("repo", &repo.name)]), true, choice, msgs);
    }
    let git_ssh_command = page.agent.as_ref().is_some_and(|a| a.git_ssh_command_set);
    if cx.use_agent || git_ssh_command || page.choice_error.is_some() {
        row(ui, "", |ui| {
            if cx.use_agent {
                help(ui, t.tr("prefs.ssh.which.agent_on.help"));
            }
            if git_ssh_command {
                warn(ui, t.tr("prefs.ssh.which.git_ssh_command.warn"));
            }
            if let Some(e) = &page.choice_error {
                warn(ui, e);
            }
        });
    }
}

/// What every level's row reads.
struct Level<'a> {
    t: Lang,
    page: &'a SshPage,
    listing: &'a SshListing,
    cx: &'a SshContext,
}

impl Level<'_> {
    /// What the closed dropdown shows for `choice`.
    fn selected_text(&self, choice: &Result<KeyChoice, String>) -> String {
        let t = self.t;
        match choice {
            Ok(KeyChoice::Unset) => t.tr("prefs.ssh.which.ssh_decides").into(),
            Ok(KeyChoice::Key(p)) => match self.listing.keys.iter().find(|k| same_spelling(&k.private, p)) {
                Some(k) => k.file_name(),
                None => t.trf("prefs.ssh.which.unlisted", &[("path", &native_path::shown(p))]),
            },
            Ok(KeyChoice::Other { .. }) => t.tr("prefs.ssh.which.custom").into(),
            Err(_) => t.tr("prefs.ssh.which.unreadable").into(),
        }
    }

    /// One level's row: its dropdown, a spinner while it is written, and its notes.
    fn row(&self, ui: &mut Ui, label: &str, repo: bool, choice: &Result<KeyChoice, String>, msgs: &mut Vec<Msg>) {
        let (t, page) = (self.t, self.page);
        let trusted = !repo || page.repo.as_ref().is_some_and(|r| r.trusted);
        let enabled = choice.is_ok() && trusted && !page.choice_busy;
        let selected = self.selected_text(choice);
        row(ui, label, |ui| {
            ui.horizontal(|ui| {
                let shown = ui.add_enabled_ui(enabled, |ui| {
                    egui::ComboBox::from_id_salt(("ssh-which-key", repo))
                        .selected_text(selected.as_str())
                        .width(fit(ui, 320.0))
                        .show_ui(ui, |ui| self.entries(ui, repo, choice, msgs))
                        .response
                });
                shown.inner.widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, enabled, format!("{label}: {selected}")));
                if page.choice_busy && page.choice_repo == repo {
                    ui.add(egui::Spinner::new().size(14.0));
                }
            });
            match choice {
                Err(e) => warn(ui, e),
                Ok(KeyChoice::Other { value, .. }) => {
                    warn(ui, &t.trf("prefs.ssh.which.other.warn", &[("value", &shown_value(value))]));
                    if self.cx.use_agent {
                        help(ui, t.tr("prefs.ssh.which.other_agent.help"));
                    }
                }
                Ok(KeyChoice::Unset | KeyChoice::Key(_)) => {}
            }
            if repo && let Some(target) = &page.repo {
                if !target.trusted {
                    help(ui, t.tr("prefs.ssh.which.not_trusted.help"));
                } else if matches!(choice, Ok(KeyChoice::Key(_) | KeyChoice::Other { .. })) {
                    help(ui, &t.trf("prefs.ssh.which.overrides.help", &[("repo", &target.name)]));
                }
            }
        });
    }

    /// The open dropdown: every listed key, then Let ssh decide. Picking the current entry does
    /// nothing; with Use local SSH agent on, only Let ssh decide can be picked.
    fn entries(&self, ui: &mut Ui, repo: bool, choice: &Result<KeyChoice, String>, msgs: &mut Vec<Msg>) {
        let why = use_blocked(self.page, self.cx.use_agent, repo, self.t);
        for k in &self.listing.keys {
            let current = matches!(choice, Ok(KeyChoice::Key(p)) if same_spelling(p, &k.private));
            let text = format!("{}  {}", k.file_name(), k.kind_label());
            let mut r = ui.add_enabled(why.is_none(), egui::SelectableLabel::new(current, text));
            if let Some(why) = why {
                r = r.on_disabled_hover_text(why);
            }
            if r.clicked() && !current {
                msgs.push(Msg::Ssh(SshMsg::UseKey { key: k.private.clone(), repo, confirmed: false }));
            }
        }
        if self.listing.keys.is_empty() {
            ui.label(RichText::new(self.t.tr("prefs.ssh.which.no_keys")).color(color::weak()));
        }
        ui.separator();
        let unset = matches!(choice, Ok(KeyChoice::Unset));
        if ui.add(egui::SelectableLabel::new(unset, self.t.tr("prefs.ssh.which.ssh_decides"))).clicked() && !unset {
            msgs.push(Msg::Ssh(SshMsg::StopUsing { repo, confirmed: false }));
        }
    }
}

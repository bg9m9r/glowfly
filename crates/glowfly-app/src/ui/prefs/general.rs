//! Preferences > General, in titled sections: Updates, Git (general_git.rs), Fetching, Graph,
//! Repositories, Sign-ins and Diagnostics.

use super::updates_row::UpdatesView;
use super::{help, row, section, warn};
use crate::keymap::{BindingId, Keymap};
use crate::logs::LogsMsg;
use crate::msg::ForgeMsg;
use crate::msg::Msg;
use crate::state::deep_link::SchemeView;
use crate::state::prefs::GitConfigValues;
use crate::ui::theme::controls;
use crate::ui::widgets::buttons;
use eframe::egui::{self, Ui};
use glowfly_core::git_version::GitInfo;
use glowfly_core::i18n::Lang;
use glowfly_core::settings::Settings;

/// The help under Watch for File Changes, naming Refresh's current key.
fn watch_files_help(t: Lang, keys: &Keymap) -> String {
    match keys.hint(BindingId::Refresh) {
        Some(k) => t.trf("prefs.general.watch_files.help_key", &[("key", &k)]),
        None => t.tr("prefs.general.watch_files.help_no_key").to_owned(),
    }
}

/// What the General page reads besides the settings it edits.
pub struct GeneralContext<'a> {
    pub program: Option<&'a GitInfo>,
    pub keys: &'a Keymap,
    pub scheme: &'a SchemeView,
    pub updates: UpdatesView<'a>,
}

pub fn general_page(ui: &mut Ui, s: &mut Settings, git: Option<&mut GitConfigValues>, cx: &GeneralContext, msgs: &mut Vec<Msg>) {
    let t = s.lang();
    section(ui, t.tr("prefs.general.section.updates"));
    super::updates_row::updates_row(ui, s, &cx.updates, msgs);
    section(ui, t.tr("prefs.general.section.git"));
    super::general_git::git_section(ui, s, git, cx.program, msgs);
    section(ui, t.tr("prefs.general.section.fetching"));
    fetching(ui, t, s);
    section(ui, t.tr("prefs.general.section.graph"));
    row(ui, t.tr("prefs.general.initial_commits.label"), |ui| {
        ui.add(egui::DragValue::new(&mut s.initial_commits).range(glowfly_core::settings::MIN_INITIAL_COMMITS..=200_000).speed(50));
        help(ui, t.tr("prefs.general.initial_commits.help"));
        warn(ui, t.tr("prefs.general.initial_commits.warn"));
    });
    row(ui, t.tr("prefs.general.load_more.label"), |ui| {
        controls::checkbox(ui, &mut s.load_more_on_scroll, t.tr("prefs.general.load_more.check"));
        help(ui, t.tr("prefs.general.load_more.help"));
    });
    row(ui, t.tr("prefs.general.index_history.label"), |ui| {
        controls::checkbox(ui, &mut s.index_history, t.tr("prefs.general.index_history.check"));
        help(ui, t.tr("prefs.general.index_history.help"));
    });
    section(ui, t.tr("prefs.general.section.repositories"));
    repositories(ui, t, s, cx);
    super::links::links_row(ui, t, cx.scheme, msgs);
    section(ui, t.tr("prefs.general.section.sign_ins"));
    row(ui, t.tr("prefs.general.remember_sign_ins.label"), |ui| {
        controls::checkbox(ui, &mut s.remember_https_credentials, t.tr("prefs.general.remember_sign_ins.check"));
        help(ui, t.tr("prefs.general.remember_sign_ins.help"));
    });
    forget_row(ui, t, s, msgs);
    section(ui, t.tr("prefs.general.section.diagnostics"));
    diagnostics(ui, t, s, msgs);
}

/// Auto-fetch, auto-prune and keeping submodules updated.
fn fetching(ui: &mut Ui, t: Lang, s: &mut Settings) {
    row(ui, t.tr("prefs.general.auto_fetch.label"), |ui| {
        ui.add(egui::DragValue::new(&mut s.auto_fetch_minutes).range(0..=60).suffix(t.tr("prefs.general.auto_fetch.suffix")));
        help(ui, t.tr("prefs.general.auto_fetch.help"));
        help(ui, t.tr("prefs.general.auto_fetch.help_lfs"));
        warn(ui, t.tr("prefs.general.auto_fetch.warn"));
    });
    row(ui, t.tr("prefs.general.auto_prune.label"), |ui| {
        controls::checkbox(ui, &mut s.auto_prune, t.tr("prefs.general.auto_prune.check"));
    });
    row(ui, t.tr("prefs.general.submodules.label"), |ui| {
        controls::checkbox(ui, &mut s.submodules.keep_updated, t.tr("prefs.general.submodules.check"));
        help(ui, t.tr("prefs.general.submodules.help"));
    });
}

/// Reopening the last tabs, watching files, the conflict check and deleting to the Trash.
fn repositories(ui: &mut Ui, t: Lang, s: &mut Settings, cx: &GeneralContext) {
    row(ui, t.tr("prefs.general.reopen.label"), |ui| {
        controls::checkbox(ui, &mut s.reopen_last_repo, t.tr("prefs.general.reopen.check"));
    });
    row(ui, t.tr("prefs.general.watch_files.label"), |ui| {
        controls::checkbox(ui, &mut s.watch_files, t.tr("prefs.general.watch_files.check"));
        help(ui, &watch_files_help(t, cx.keys));
    });
    row(ui, t.tr("prefs.general.conflict_check.label"), |ui| {
        controls::checkbox(ui, &mut s.conflict_check, t.tr("prefs.general.conflict_check.check"));
        help(ui, t.tr("prefs.general.conflict_check.help"));
    });
    row(ui, t.tr("prefs.general.delete_to_trash.label"), |ui| {
        let check = if cfg!(windows) {
            t.tr("prefs.general.delete_to_trash.check_windows")
        } else {
            t.tr("prefs.general.delete_to_trash.check_other")
        };
        controls::checkbox(ui, &mut s.delete_to_trash, check);
        help(ui, t.tr("prefs.general.delete_to_trash.help"));
    });
    row(ui, t.tr("prefs.general.auto_stash.label"), |ui| {
        controls::checkbox(ui, &mut s.auto_stash, t.tr("prefs.general.auto_stash.check"));
        help(ui, t.tr("prefs.general.auto_stash.help"));
    });
}

/// Extended logging and the logs folder.
fn diagnostics(ui: &mut Ui, t: Lang, s: &Settings, msgs: &mut Vec<Msg>) {
    row(ui, t.tr("prefs.general.extended_logging.label"), |ui| {
        // Through a message, so the runner hears of it (the error log's level changes too).
        let mut on = s.extended_logging;
        if controls::checkbox(ui, &mut on, t.tr("prefs.general.extended_logging.check")).changed() {
            msgs.push(Msg::Logs(LogsMsg::SetExtended(on)));
        }
        help(ui, t.tr("prefs.general.extended_logging.help"));
    });
    row(ui, t.tr("prefs.general.logs.label"), |ui| {
        if ui.add(buttons::secondary(t.tr("prefs.general.logs.button"))).clicked() {
            msgs.push(Msg::Logs(LogsMsg::OpenFolder));
        }
        help(ui, t.tr("prefs.general.logs.help"));
    });
}

/// Forget All Connections: forgetting the saved tokens, or those and Git Credential Manager's
/// sign-ins (after asking).
fn forget_row(ui: &mut Ui, t: Lang, s: &Settings, msgs: &mut Vec<Msg>) {
    row(ui, t.tr("prefs.general.forget.label"), |ui| {
        let n = s.connections.len();
        ui.horizontal_wrapped(|ui| {
            if ui.add(buttons::danger(t.tr("prefs.general.forget.button")).enabled(n > 0)).clicked() {
                msgs.push(Msg::Forge(ForgeMsg::ForgetTokens { gcm: false }));
            }
            let gcm_hosts = s.connections.iter().filter(|c| c.account.provider.supports_gcm()).count();
            if ui.add(buttons::danger(t.tr("prefs.general.forget.gcm_button")).enabled(gcm_hosts > 0)).clicked() {
                msgs.push(Msg::confirm_danger(
                    t.tr("prefs.general.forget.confirm_title"),
                    t.trn("prefs.general.forget.gcm_body", gcm_hosts, &[]),
                    t.tr("prefs.general.forget.button"),
                    Msg::Forge(ForgeMsg::ForgetTokens { gcm: true }),
                ));
            }
        });
        if n == 0 {
            help(ui, t.tr("prefs.general.forget.none"));
        } else {
            help(ui, &t.trn("prefs.general.forget.connections", n, &[]));
        }
    });
}

#[cfg(test)]
#[path = "../snapshot_tests_language.rs"]
mod snapshot_tests_language;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::{Chord, KeyName};

    #[test]
    fn watch_help_names_refreshs_current_key() {
        let mut keys = Keymap::default();
        keys.set(BindingId::Refresh, vec![Chord::plain(KeyName::F(9))]);
        assert!(watch_files_help(Lang::En, &keys).ends_with("or you press F9."));
        keys.set(BindingId::Refresh, Vec::new());
        assert!(watch_files_help(Lang::En, &keys).ends_with("or you choose Refresh."));
    }
}

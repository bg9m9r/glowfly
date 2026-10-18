//! Preferences > General.

use super::updates_row::UpdatesView;
use super::{help, row, text_field, warn};
use crate::keymap::{BindingId, Keymap};
use crate::logs::LogsMsg;
use crate::msg::ForgeMsg;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::state::deep_link::SchemeView;
use crate::state::prefs::{Autocrlf, GitConfigValues};
use crate::ui::theme::color;
use eframe::egui::{self, Color32, RichText, Ui};
use trellis_core::git_version::GitInfo;
use trellis_core::i18n::Lang;
use trellis_core::settings::Settings;

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
    let (program, keys, scheme) = (cx.program, cx.keys, cx.scheme);
    let t = s.lang();
    super::language::language_row(ui, s);
    row(ui, t.tr("prefs.general.git.label"), |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(git_program_text(t, program)).color(color::text()));
            if ui.button(t.tr("prefs.general.git.button")).clicked() {
                msgs.push(Msg::GitEnv(crate::git_env::GitEnvMsg::ShowPage));
            }
        });
        help(ui, t.tr("prefs.general.git.help"));
    });
    super::updates_row::updates_row(ui, s, &cx.updates, msgs);
    row(ui, t.tr("prefs.general.auto_fetch.label"), |ui| {
        ui.add(egui::DragValue::new(&mut s.auto_fetch_minutes).range(0..=60).suffix(t.tr("prefs.general.auto_fetch.suffix")));
        help(ui, t.tr("prefs.general.auto_fetch.help"));
        help(ui, t.tr("prefs.general.auto_fetch.help_lfs"));
        warn(ui, t.tr("prefs.general.auto_fetch.warn"));
    });
    row(ui, t.tr("prefs.general.auto_prune.label"), |ui| {
        ui.checkbox(&mut s.auto_prune, t.tr("prefs.general.auto_prune.check"));
    });
    row(ui, t.tr("prefs.general.remember_sign_ins.label"), |ui| {
        ui.checkbox(&mut s.remember_https_credentials, t.tr("prefs.general.remember_sign_ins.check"));
        help(ui, t.tr("prefs.general.remember_sign_ins.help"));
    });
    row(ui, t.tr("prefs.general.default_branch.label"), |ui| {
        text_field(ui, &mut s.default_branch, "main");
        help(ui, t.tr("prefs.general.default_branch.help"));
    });
    row(ui, t.tr("prefs.general.initial_commits.label"), |ui| {
        ui.add(egui::DragValue::new(&mut s.initial_commits).range(trellis_core::settings::MIN_INITIAL_COMMITS..=200_000).speed(50));
        help(ui, t.tr("prefs.general.initial_commits.help"));
        warn(ui, t.tr("prefs.general.initial_commits.warn"));
    });
    row(ui, t.tr("prefs.general.load_more.label"), |ui| {
        ui.checkbox(&mut s.load_more_on_scroll, t.tr("prefs.general.load_more.check"));
        help(ui, t.tr("prefs.general.load_more.help"));
    });
    row(ui, t.tr("prefs.general.reopen.label"), |ui| {
        ui.checkbox(&mut s.reopen_last_repo, t.tr("prefs.general.reopen.check"));
    });
    row(ui, t.tr("prefs.general.watch_files.label"), |ui| {
        ui.checkbox(&mut s.watch_files, t.tr("prefs.general.watch_files.check"));
        help(ui, &watch_files_help(t, keys));
    });
    row(ui, t.tr("prefs.general.conflict_check.label"), |ui| {
        ui.checkbox(&mut s.conflict_check, t.tr("prefs.general.conflict_check.check"));
        help(ui, t.tr("prefs.general.conflict_check.help"));
    });
    row(ui, t.tr("prefs.general.submodules.label"), |ui| {
        ui.checkbox(&mut s.submodules.keep_updated, t.tr("prefs.general.submodules.check"));
        help(ui, t.tr("prefs.general.submodules.help"));
    });
    row(ui, t.tr("prefs.general.delete_to_trash.label"), |ui| {
        let check = if cfg!(windows) {
            t.tr("prefs.general.delete_to_trash.check_windows")
        } else {
            t.tr("prefs.general.delete_to_trash.check_other")
        };
        ui.checkbox(&mut s.delete_to_trash, check);
        help(ui, t.tr("prefs.general.delete_to_trash.help"));
    });
    row(ui, t.tr("prefs.general.extended_logging.label"), |ui| {
        // Through a message, so the runner hears of it (the error log's level changes too).
        let mut on = s.extended_logging;
        if ui.checkbox(&mut on, t.tr("prefs.general.extended_logging.check")).changed() {
            msgs.push(Msg::Logs(LogsMsg::SetExtended(on)));
        }
        help(ui, t.tr("prefs.general.extended_logging.help"));
    });
    row(ui, t.tr("prefs.general.logs.label"), |ui| {
        if ui.button(t.tr("prefs.general.logs.button")).clicked() {
            msgs.push(Msg::Logs(LogsMsg::OpenFolder));
        }
        help(ui, t.tr("prefs.general.logs.help"));
    });
    if let Some(git) = git {
        row(ui, t.tr("prefs.general.longpaths.label"), |ui| {
            if ui.checkbox(&mut git.longpaths, "").changed() {
                msgs.push(Msg::Prefs(PrefsMsg::GitConfig {
                    global: true,
                    key: "core.longpaths".into(),
                    value: Some(git.longpaths.to_string()),
                }));
            }
            help(ui, t.tr("prefs.general.longpaths.help"));
        });
        row(ui, t.tr("prefs.general.autocrlf.label"), |ui| {
            let before = git.autocrlf.clone();
            egui::ComboBox::from_id_salt("autocrlf").width(320.0).selected_text(git.autocrlf.label(t)).show_ui(ui, |ui| {
                for (choice, label) in Autocrlf::choices(t) {
                    ui.selectable_value(&mut git.autocrlf, choice, label);
                }
            });
            if git.autocrlf != before {
                msgs.push(Msg::Prefs(PrefsMsg::GitConfig { global: true, key: "core.autocrlf".into(), value: git.autocrlf.value() }));
            }
            help(ui, t.tr("prefs.general.autocrlf.help"));
        });
    }
    forget_row(ui, t, s, msgs);
    super::links::links_row(ui, t, scheme, msgs);
}

/// Forget All Connections: forgetting the saved tokens, or those and Git Credential Manager's
/// sign-ins (after asking).
fn forget_row(ui: &mut Ui, t: Lang, s: &Settings, msgs: &mut Vec<Msg>) {
    row(ui, t.tr("prefs.general.forget.label"), |ui| {
        let n = s.connections.len();
        let danger = |text: &str, on: bool| {
            egui::Button::new(RichText::new(text).color(if on { Color32::WHITE } else { color::faint() })).fill(if on {
                color::danger()
            } else {
                color::danger_off()
            })
        };
        ui.horizontal(|ui| {
            if ui.add_enabled(n > 0, danger(t.tr("prefs.general.forget.button"), n > 0)).clicked() {
                msgs.push(Msg::Forge(ForgeMsg::ForgetTokens { gcm: false }));
            }
            let gcm_hosts = s.connections.iter().filter(|c| c.account.provider.supports_gcm()).count();
            if ui.add_enabled(gcm_hosts > 0, danger(t.tr("prefs.general.forget.gcm_button"), gcm_hosts > 0)).clicked() {
                msgs.push(Msg::confirm(
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

/// "git 2.45.1 at C:\Program Files\Git\cmd\git.exe", or what is missing.
pub(super) fn git_program_text(t: Lang, info: Option<&GitInfo>) -> String {
    let Some(info) = info else { return t.tr("prefs.general.git_program.checking").to_owned() };
    let at = info.path.as_deref().map(trellis_core::native_path::shown);
    match (info.version, at) {
        (Some(v), Some(at)) => t.trf("prefs.general.git_program.at", &[("version", &trellis_core::git_version::show(v)), ("path", &at)]),
        (Some(v), None) => t.trf("prefs.general.git_program.version", &[("version", &trellis_core::git_version::show(v))]),
        (None, Some(at)) => t.trf("prefs.general.git_program.broken", &[("path", &at)]),
        (None, None) => t.tr("prefs.general.git_program.missing").to_owned(),
    }
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

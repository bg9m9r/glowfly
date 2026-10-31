//! Preferences > General > Git: the git program Glowfly runs (chosen on the Git page), the
//! default branch name for new repositories, and two global git config values, long paths and
//! line endings. Those two are edited in place and written by the Save / Revert bar.

use super::{check_row, help, row, text_field};
use crate::msg::{Msg, PrefsMsg};
use crate::state::PrefPage;
use crate::state::prefs::{Autocrlf, GitConfigValues};
use crate::ui::theme::color;
use crate::ui::widgets::buttons;
use eframe::egui::{self, RichText, Ui};
use glowfly_core::git_version::GitInfo;
use glowfly_core::i18n::Lang;
use glowfly_core::settings::Settings;

pub fn git_section(ui: &mut Ui, s: &mut Settings, git: Option<&mut GitConfigValues>, program: Option<&GitInfo>, msgs: &mut Vec<Msg>) {
    let t = s.lang();
    row(ui, t.tr("prefs.general.git.label"), |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(git_program_text(t, program)).color(color::text()));
            if ui.add(buttons::secondary(t.tr("prefs.general.git.button"))).clicked() {
                msgs.push(Msg::Prefs(PrefsMsg::Navigate(PrefPage::Git)));
            }
        });
        help(ui, t.tr("prefs.general.git.help"));
    });
    row(ui, t.tr("prefs.general.default_branch.label"), |ui| {
        text_field(ui, &mut s.default_branch, "main");
        help(ui, t.tr("prefs.general.default_branch.help"));
    });
    let Some(git) = git else { return };
    let (label, check, text) =
        (t.tr("prefs.general.longpaths.label"), t.tr("prefs.general.longpaths.check"), t.tr("prefs.general.longpaths.help"));
    check_row(ui, label, &mut git.longpaths, check, Some(text));
    row(ui, t.tr("prefs.general.autocrlf.label"), |ui| {
        egui::ComboBox::from_id_salt("autocrlf").width(super::fit(ui, 320.0)).selected_text(git.autocrlf.label(t)).show_ui(ui, |ui| {
            for (choice, label) in Autocrlf::choices(t) {
                ui.selectable_value(&mut git.autocrlf, choice, label);
            }
        });
        help(ui, t.tr("prefs.general.autocrlf.help"));
    });
}

/// "git 2.45.1 at C:\Program Files\Git\cmd\git.exe", or what is missing.
pub(super) fn git_program_text(t: Lang, info: Option<&GitInfo>) -> String {
    let Some(info) = info else { return t.tr("prefs.general.git_program.checking").to_owned() };
    let at = info.path.as_deref().map(glowfly_core::native_path::shown);
    match (info.version, at) {
        (Some(v), Some(at)) => t.trf("prefs.general.git_program.at", &[("version", &glowfly_core::git_version::show(v)), ("path", &at)]),
        (Some(v), None) => t.trf("prefs.general.git_program.version", &[("version", &glowfly_core::git_version::show(v))]),
        (None, Some(at)) => t.trf("prefs.general.git_program.broken", &[("path", &at)]),
        (None, None) => t.tr("prefs.general.git_program.missing").to_owned(),
    }
}

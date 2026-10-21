//! Preferences > Git: which git program Glowfly runs, the gits found on this machine and the
//! version in use. The page only asks (`GitEnvMsg`); update checks a choice, saves it and
//! follows the new git everywhere.

use super::general_git::git_program_text;
use super::{help, row, warn};
use crate::git_env::GitEnvMsg;
use crate::msg::Msg;
use crate::state::git_env::GitState;
use crate::ui::theme::color;
use crate::ui::theme::controls;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{self, RichText, Ui};
use glowfly_core::git_program::{Candidate, from_setting};
use glowfly_core::git_version::{Version, full_support, missing_for, show};
use glowfly_core::i18n::Lang;
use glowfly_core::native_path::shown;
use glowfly_core::paths::same_spelling;
use glowfly_core::settings::Settings;
use std::path::PathBuf;

const HELP: &str = "Glowfly runs this git for commits, fetches, pushes, hooks and signing. The built-in terminal and external \
                    tools still use whatever git your PATH finds.";

pub fn git_program_page(ui: &mut Ui, s: &Settings, git: &GitState, msgs: &mut Vec<Msg>) {
    let page = &git.page;
    let current = from_setting(s.git_program.as_deref());
    let busy = page.checking.is_some();
    let choose = |msgs: &mut Vec<Msg>, p: Option<PathBuf>| msgs.push(Msg::GitEnv(GitEnvMsg::Choose(p)));
    row(ui, "Git program", |ui| {
        let on_path = current.is_none();
        if radio(ui, !busy, on_path, "Use git from your PATH") && !on_path {
            choose(msgs, None);
        }
        if let Some(text) = path_text(s.lang(), git) {
            under(ui, |ui| help(ui, &text));
        }
        let others: Vec<&Candidate> = page.candidates.iter().flatten().filter(|c| !c.first_on_path).collect();
        for c in &others {
            let selected = current.as_deref().is_some_and(|p| same_spelling(p, &c.path));
            if radio(ui, !busy && c.check.is_ok(), selected, &shown(&c.path)) && !selected {
                choose(msgs, Some(c.path.clone()));
            }
            under(ui, |ui| match &c.check {
                Ok(v) => version_lines(ui, *v),
                Err(why) => {
                    ui.add(egui::Label::new(RichText::new(sentence(why)).size(type_scale::SMALL).color(color::danger())).wrap());
                }
            });
        }
        // A git picked with Browse… earlier, which the scan doesn't list.
        if let Some(p) = current.as_deref().filter(|p| !others.iter().any(|c| same_spelling(p, &c.path))) {
            radio(ui, !busy, true, &shown(p));
            if let Some(v) =
                git.info.as_ref().filter(|i| i.chosen && i.path.as_deref().is_some_and(|q| same_spelling(p, q))).and_then(|i| i.version)
            {
                under(ui, |ui| version_lines(ui, v));
            }
        }
        if page.candidates.is_none() {
            spinner(ui, "Looking for git installations…");
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.add(buttons::secondary("Browse…").enabled(!busy)).clicked() {
                msgs.push(Msg::GitEnv(GitEnvMsg::Browse));
            }
            if ui.add(buttons::secondary("Look again").enabled(page.candidates.is_some())).clicked() {
                msgs.push(Msg::GitEnv(GitEnvMsg::Rescan));
            }
        });
        if let Some(c) = &page.checking {
            let what = c.program.as_deref().map(shown).unwrap_or_else(|| "git from your PATH".to_owned());
            spinner(ui, &format!("Checking {what}…"));
        }
        if let Some((what, why)) = &page.refusal {
            warn(ui, &format!("Can't use {what}: {why}"));
        }
        help(ui, HELP);
    });
    row(ui, "Version", |ui| {
        ui.label(RichText::new(git_program_text(s.lang(), git.info.as_ref())).color(color::text()));
        let missing = git.info.as_ref().map(|i| i.missing()).unwrap_or_default();
        if !missing.is_empty() {
            let (major, minor) = full_support();
            let names: Vec<&str> = missing.iter().map(|f| f.label()).collect();
            warn(ui, &format!("Some features need git {major}.{minor} or newer: {}.", names.join(", ")));
        }
    });
}

/// What "Use git from your PATH" runs: the scan's PATH git, else what the startup check found
/// while that is the PATH git. None while neither knows.
fn path_text(t: Lang, git: &GitState) -> Option<String> {
    if let Some(c) = git.page.path_git() {
        return Some(match &c.check {
            Ok(v) => format!("git {} at {}", show(*v), shown(&c.path)),
            Err(why) => format!("{}: {why}", shown(&c.path)),
        });
    }
    match &git.info {
        Some(info) if !info.chosen && info.path.is_some() => Some(git_program_text(t, Some(info))),
        _ if git.page.candidates.is_some() => Some(t.tr("prefs.general.git_program.missing").to_owned()),
        _ => None,
    }
}

/// "git 2.34.1", and for a git too old for some features, which ones will be off.
fn version_lines(ui: &mut Ui, v: Version) {
    help(ui, &format!("git {}", show(v)));
    let missing = missing_for(Some(v));
    if !missing.is_empty() {
        let (major, minor) = full_support();
        let names: Vec<&str> = missing.iter().map(|f| f.label()).collect();
        warn(ui, &format!("Older than {major}.{minor}, so these will be off: {}.", names.join(", ")));
    }
}

/// `why` (a reason that ends "Can't use <path>: ...") as a sentence of its own.
fn sentence(why: &str) -> String {
    let mut chars = why.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect::<String>() + ".").unwrap_or_default()
}

/// A radio row; true when clicked.
fn radio(ui: &mut Ui, enabled: bool, selected: bool, label: &str) -> bool {
    ui.add_enabled(enabled, controls::Radio::new(selected, RichText::new(label).color(color::text()))).clicked()
}

/// Lines under a radio row, lined up with its label.
fn under(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_top(|ui| {
        ui.add_space(22.0);
        ui.vertical(|ui| add(ui));
    });
}

fn spinner(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add(egui::Spinner::new());
        ui.label(RichText::new(text).color(color::weak()));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use glowfly_core::git_version::GitInfo;
    use std::path::Path;

    fn candidate(path: &str, first_on_path: bool, check: Result<Version, String>) -> Candidate {
        Candidate { path: Path::new(path).to_path_buf(), first_on_path, check }
    }

    #[test]
    fn the_path_row_says_what_path_runs() {
        let mut git = GitState::default();
        assert_eq!(path_text(Lang::En, &git), None, "nothing known yet");
        git.info = Some(GitInfo { path: Some("/usr/bin/git".into()), version: Some((2, 39, 3)), chosen: false });
        assert!(path_text(Lang::En, &git).unwrap().starts_with("git 2.39.3 at "));
        git.info = Some(GitInfo { path: Some("/opt/git".into()), version: Some((2, 45, 1)), chosen: true });
        assert_eq!(path_text(Lang::En, &git), None, "the chosen git isn't the PATH one");
        git.page.candidates = Some(Vec::new());
        assert_eq!(path_text(Lang::En, &git).as_deref(), Some("Not found on your PATH"));
        git.page.candidates = Some(vec![candidate("/usr/local/bin/git", true, Ok((2, 45, 1)))]);
        assert!(path_text(Lang::En, &git).unwrap().starts_with("git 2.45.1 at "));
    }

    #[test]
    fn a_reason_reads_as_a_sentence() {
        assert_eq!(sentence("it didn't answer"), "It didn't answer.");
        assert_eq!(sentence(""), "");
    }
}

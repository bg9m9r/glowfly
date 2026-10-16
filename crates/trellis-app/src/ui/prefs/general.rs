//! Preferences > General.

use super::{help, row, text_field, warn};
use crate::msg::ForgeMsg;
use crate::msg::Msg;
use crate::msg::PrefsMsg;
use crate::state::files::TRASH;
use crate::state::prefs::{Autocrlf, GitConfigValues};
use crate::ui::theme::color;
use eframe::egui::{self, Color32, RichText, Ui};
use trellis_core::git_version::GitInfo;
use trellis_core::settings::Settings;

pub fn general_page(ui: &mut Ui, s: &mut Settings, git: Option<&mut GitConfigValues>, program: Option<&GitInfo>, msgs: &mut Vec<Msg>) {
    row(ui, "Git", |ui| {
        ui.label(RichText::new(git_program_text(program)).color(color::text()));
        help(ui, "The git program Trellis runs for commits, fetches, pushes and hooks: the first one on your PATH.");
    });
    row(ui, "Auto-Fetch Interval", |ui| {
        ui.add(egui::DragValue::new(&mut s.auto_fetch_minutes).range(0..=60).suffix(" min"));
        help(ui, "In minutes, between 0 and 60 (0 disables auto-fetch). Fetches all remotes of the open repository.");
        warn(ui, "If you have many remotes, this may impact performance.");
    });
    row(ui, "Auto-Prune", |ui| {
        ui.checkbox(&mut s.auto_prune, "Remove remote-tracking branches that no longer exist on the remote when fetching");
    });
    row(ui, "Default Branch Name", |ui| {
        text_field(ui, &mut s.default_branch, "main");
        help(ui, "Sets the default branch name when initializing new repositories.");
    });
    row(ui, "Initial Commits in Graph", |ui| {
        ui.add(egui::DragValue::new(&mut s.initial_commits).range(trellis_core::settings::MIN_INITIAL_COMMITS..=200_000).speed(50));
        help(ui, "Minimum 500. Takes effect when you leave Preferences or refresh.");
        warn(ui, "Increasing this value may adversely affect performance.");
    });
    row(ui, "Load More While Scrolling", |ui| {
        ui.checkbox(&mut s.load_more_on_scroll, "Load more commits when you scroll near the end of the graph");
        help(ui, "When off, only the graph's Load more link adds commits past the limit.");
    });
    row(ui, "Reopen Last Repository", |ui| {
        ui.checkbox(&mut s.reopen_last_repo, "Reopen the tabs from last time when Trellis starts");
    });
    row(ui, "Watch for File Changes", |ui| {
        ui.checkbox(&mut s.watch_files, "Refresh the working tree as soon as files change outside Trellis");
        help(ui, "When off, Trellis refreshes when its window regains focus or you press F5.");
    });
    row(ui, "Conflict Prediction", |ui| {
        ui.checkbox(&mut s.conflict_check, "Check the current branch for conflicts with its upstream or default branch");
        help(
            ui,
            "Runs after each refresh and fetch with git merge-tree, without touching your files. Needs git 2.38 or newer. \
             Branch menus can still check on demand when this is off.",
        );
    });
    row(ui, "Deleting Files", |ui| {
        ui.checkbox(&mut s.delete_to_trash, format!("Move deleted files to the {TRASH}"));
        help(ui, "When off, Delete… and discarding an untracked file remove it from disk straight away, and it can't be recovered.");
    });
    if let Some(git) = git {
        row(ui, "Longpaths", |ui| {
            if ui.checkbox(&mut git.longpaths, "").changed() {
                msgs.push(Msg::Prefs(PrefsMsg::GitConfig {
                    global: true,
                    key: "core.longpaths".into(),
                    value: Some(git.longpaths.to_string()),
                }));
            }
            help(
                ui,
                "Sets core.longpaths in your global Git config. By default, Windows only supports file paths less than 260 characters. Enable support for longer file paths.",
            );
        });
        row(ui, "AutoCRLF", |ui| {
            let before = git.autocrlf.clone();
            egui::ComboBox::from_id_salt("autocrlf").width(320.0).selected_text(git.autocrlf.label()).show_ui(ui, |ui| {
                for (choice, label) in Autocrlf::CHOICES {
                    ui.selectable_value(&mut git.autocrlf, choice, label);
                }
            });
            if git.autocrlf != before {
                msgs.push(Msg::Prefs(PrefsMsg::GitConfig { global: true, key: "core.autocrlf".into(), value: git.autocrlf.value() }));
            }
            help(ui, "Sets core.autocrlf in your global Git config: how line endings are converted between your files and commits.");
        });
    }
    row(ui, "Forget All Connections", |ui| {
        let n = s.connections.len();
        let danger = |text: &str, on: bool| {
            egui::Button::new(RichText::new(text).color(if on { Color32::WHITE } else { color::faint() })).fill(if on {
                color::danger()
            } else {
                color::danger_off()
            })
        };
        ui.horizontal(|ui| {
            if ui.add_enabled(n > 0, danger("Forget All", n > 0)).clicked() {
                msgs.push(Msg::Forge(ForgeMsg::ForgetTokens { gcm: false }));
            }
            let gcm_hosts = s.connections.iter().filter(|c| c.account.provider.supports_gcm()).count();
            if ui.add_enabled(gcm_hosts > 0, danger("Forget All + Git Credential Manager", gcm_hosts > 0)).clicked() {
                msgs.push(Msg::confirm(
                    "Sign out of Git Credential Manager",
                    format!(
                        "This disconnects every integration and also removes the sign-ins Git Credential Manager stored for {gcm_hosts} host{}. \
                         git push and pull to those hosts will ask you to sign in again.",
                        if gcm_hosts == 1 { "" } else { "s" }
                    ),
                    "Forget All",
                    Msg::Forge(ForgeMsg::ForgetTokens { gcm: true }),
                ));
            }
        });
        if n == 0 {
            help(ui, "Trellis has no stored connections to remove.");
        } else {
            help(
                ui,
                &format!(
                    "Disconnects {n} integration{} and deletes their saved tokens. The second button also signs Git Credential Manager out of those hosts.",
                    if n == 1 { "" } else { "s" }
                ),
            );
        }
    });
}

/// "git 2.45.1 at C:\Program Files\Git\cmd\git.exe", or what is missing.
pub(crate) fn git_program_text(info: Option<&GitInfo>) -> String {
    let Some(info) = info else { return "Checking…".to_owned() };
    let at = info.path.as_deref().map(trellis_core::native_path::shown);
    match (info.version, at) {
        (Some(v), Some(at)) => format!("git {} at {at}", trellis_core::git_version::show(v)),
        (Some(v), None) => format!("git {}", trellis_core::git_version::show(v)),
        (None, Some(at)) => format!("{at} doesn't run"),
        (None, None) => "Not found on your PATH".to_owned(),
    }
}

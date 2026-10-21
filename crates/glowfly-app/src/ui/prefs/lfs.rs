//! Preferences > Git LFS (repository-specific): whether LFS is set up, the tracked patterns,
//! a field to track a new one, and the LFS commands. While git is held in the repository
//! (trust) only the root .gitattributes' patterns show, with Review trust…. Every action is a
//! `Msg`.

use super::{help, primary_button, row, text_field, warn};
use crate::lfs::LfsMsg;
use crate::msg::FilesMsg;
use crate::msg::Msg;
use crate::msg::TrustMsg;
use crate::state::blocked::BUSY;
use crate::state::lfs::{INSTALL_URL, LfsPage};
use crate::ui::theme::color;
use crate::ui::theme::type_scale;
use crate::ui::widgets::buttons;
use eframe::egui::{self, Key, RichText, Ui};
use glowfly_core::lfs::{LfsCommand, LfsPattern, LfsStatus};
use std::path::{Path, PathBuf};

/// What the page needs from outside Preferences, cloned before the page borrows its state.
pub struct LfsContext {
    pub repo_name: Option<String>,
    /// Where LFS push goes; None when the repository has no remote.
    pub remote: Option<String>,
    /// The running job's label.
    pub job: Option<String>,
    /// The active repository's real path while git is held there, for Review trust….
    pub held: Option<PathBuf>,
    /// The root .gitattributes' patterns, read without git.
    pub patterns: Vec<LfsPattern>,
}

pub fn lfs_page(ui: &mut Ui, page: &mut LfsPage, cx: &LfsContext, msgs: &mut Vec<Msg>) {
    let Some(repo) = cx.repo_name.as_deref() else {
        help(ui, "Open a repository to set up Git LFS.");
        return;
    };
    if let Some(real) = &cx.held {
        return held(ui, real, &cx.patterns, msgs);
    }
    if let Some(error) = &page.error {
        warn(ui, error);
        ui.add_space(6.0);
        if ui.add(buttons::secondary("Check again")).clicked() {
            msgs.push(Msg::Lfs(LfsMsg::Reload));
        }
        return;
    }
    let Some(status) = page.status.clone() else {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new());
            ui.label(RichText::new("Checking Git LFS…").color(color::weak()));
        });
        return;
    };
    let busy = cx.job.is_some();
    match &status.version {
        None => missing(ui, &status, msgs),
        Some(_) if !status.initialized => not_set_up(ui, &status, repo, busy, msgs),
        Some(version) => set_up(ui, page, &status, version, cx, msgs),
    }
}

/// Git is held in the repository: nothing was read with it, and no command runs until it is
/// trusted. The patterns come from .gitattributes.
fn held(ui: &mut Ui, real: &Path, patterns: &[LfsPattern], msgs: &mut Vec<Msg>) {
    row(ui, "Status", |ui| {
        ui.label(RichText::new("Paused until you trust this repository.").color(color::text()));
        help(
            ui,
            "Glowfly reads the LFS setup with git, and git would run the programs this repository's own settings and hooks name. \
             LFS commands wait for your decision too.",
        );
        ui.add_space(4.0);
        if ui.add(buttons::secondary("Review trust…")).clicked() {
            msgs.push(Msg::Trust(TrustMsg::Ask { real: real.to_path_buf() }));
        }
    });
    row(ui, "Tracked patterns", |ui| {
        if patterns.is_empty() {
            help(ui, "No patterns in .gitattributes.");
        } else {
            pattern_list(ui, patterns, false, true, msgs);
            help(ui, "Read from .gitattributes.");
        }
    });
}

/// git-lfs isn't installed: say so, and warn when the repository already uses LFS.
fn missing(ui: &mut Ui, status: &LfsStatus, msgs: &mut Vec<Msg>) {
    row(ui, "Status", |ui| {
        warn(ui, "Git LFS isn't installed, so Glowfly can't run LFS commands here.");
        help(ui, "Install it from git-lfs.com, then choose Check again.");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.add(buttons::secondary("Get Git LFS")).clicked() {
                msgs.push(Msg::OpenUrl(INSTALL_URL.into()));
            }
            if ui.add(buttons::secondary("Check again")).clicked() {
                msgs.push(Msg::Lfs(LfsMsg::Reload));
            }
        });
    });
    if !status.patterns.is_empty() {
        row(ui, "Tracked patterns", |ui| {
            let n = status.patterns.len();
            let count = if n == 1 { "1 pattern".to_owned() } else { format!("{n} patterns") };
            warn(
                ui,
                &format!(
                    "This repository stores {count} in LFS. Without Git LFS, those files are checked out as small pointer files, and \
                     changing and committing them can break them for everyone."
                ),
            );
            ui.add_space(4.0);
            pattern_list(ui, &status.patterns, false, true, msgs);
        });
    }
}

/// git-lfs is installed but this repository isn't set up.
fn not_set_up(ui: &mut Ui, status: &LfsStatus, repo: &str, busy: bool, msgs: &mut Vec<Msg>) {
    row(ui, "Status", |ui| {
        ui.label(RichText::new(format!("Git LFS isn't set up in {repo}.")).color(color::text()));
        help(
            ui,
            "Setting up installs the LFS hooks and filters for this repository only (git lfs install --local). Without the \
             pre-push hook, pushes don't upload LFS files.",
        );
        if !status.foreign_hooks.is_empty() {
            warn(
                ui,
                &format!(
                    "These hooks already exist and aren't Git LFS hooks: {}. Setting up will stop without changing them; merge them \
                     by hand (git lfs update --manual explains how).",
                    status.foreign_hooks.join(", ")
                ),
            );
        }
        ui.add_space(4.0);
        if primary_button(ui, "Initialize LFS", !busy, busy.then_some(BUSY)).clicked() {
            msgs.push(Msg::Lfs(LfsMsg::Initialize));
        }
    });
    if !status.patterns.is_empty() {
        row(ui, "Tracked patterns", |ui| {
            pattern_list(ui, &status.patterns, false, true, msgs);
            help(ui, "Initialize LFS to work with these files.");
        });
    }
}

/// The repository is set up: patterns, the new-pattern field, .gitattributes and commands.
fn set_up(ui: &mut Ui, page: &mut LfsPage, status: &LfsStatus, version: &str, cx: &LfsContext, msgs: &mut Vec<Msg>) {
    let busy = cx.job.is_some();
    row(ui, "Status", |ui| {
        ui.label(RichText::new(format!("Enabled · git-lfs {version}")).color(color::text()));
    });
    row(ui, "Tracked patterns", |ui| {
        if status.patterns.is_empty() {
            help(ui, "No patterns yet.");
        } else {
            pattern_list(ui, &status.patterns, true, busy, msgs);
        }
        help(
            ui,
            "Files that match a pattern are stored in LFS from their next commit. Files already committed stay in history as they are.",
        );
    });
    row(ui, "New pattern", |ui| {
        let mut submit = false;
        ui.horizontal(|ui| {
            // Always enabled: disabling it while a job (an auto-fetch, say) runs would take the
            // focus away mid-typing. Only Track waits for the job.
            let r = text_field(ui, &mut page.new_pattern, "*.psd, *.zip or assets/video/**");
            if r.changed() {
                page.pattern_error = None;
            }
            submit = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            submit |= ui.add(buttons::secondary("Track").enabled(!busy)).clicked();
        });
        if submit && !busy {
            msgs.push(Msg::Lfs(LfsMsg::Track(page.new_pattern.clone())));
        }
        if let Some(e) = &page.pattern_error {
            warn(ui, e);
        }
    });
    row(ui, ".gitattributes", |ui| {
        if ui.add(buttons::secondary("Open .gitattributes").enabled(status.has_gitattributes)).clicked() {
            msgs.push(Msg::Files(FilesMsg::OpenWorkingFile(".gitattributes".into())));
        }
        if !status.has_gitattributes {
            help(ui, "Created when you track the first pattern.");
        }
    });
    row(ui, "Commands", |ui| commands_row(ui, cx, msgs));
}

/// One line per pattern; `editable` adds an Untrack button (disabled while `busy`).
fn pattern_list(ui: &mut Ui, patterns: &[LfsPattern], editable: bool, busy: bool, msgs: &mut Vec<Msg>) {
    for p in patterns {
        ui.horizontal(|ui| {
            ui.set_min_height(22.0);
            ui.label(RichText::new(&p.display).monospace().color(color::text()));
            if p.lockable {
                ui.label(RichText::new("lockable").size(type_scale::CAPTION).color(color::faint()));
            }
            if editable {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(buttons::secondary("Untrack").small().enabled(!busy)).clicked() {
                        msgs.push(Msg::Lfs(LfsMsg::Untrack(p.raw.clone())));
                    }
                });
            }
        });
    }
}

fn hover(command: LfsCommand, remote: Option<&str>) -> String {
    match command {
        LfsCommand::Fetch => {
            "Download the LFS files the checked-out commit needs into the local LFS store. The working tree is not changed.".into()
        }
        LfsCommand::Pull => "Fetch, then replace pointer files in the working tree with their contents.".into(),
        LfsCommand::Push => format!("Upload every LFS file that local branches use to {}.", remote.unwrap_or("the remote")),
        LfsCommand::Prune => "Delete old local LFS files that recent commits don't use and the remote already has.".into(),
        LfsCommand::Checkout => {
            "Replace pointer files in the working tree with contents already in the local store. Nothing is downloaded.".into()
        }
    }
}

fn commands_row(ui: &mut Ui, cx: &LfsContext, msgs: &mut Vec<Msg>) {
    let busy = cx.job.is_some();
    ui.horizontal_wrapped(|ui| {
        for command in LfsCommand::ALL {
            let no_remote = command.needs_remote() && cx.remote.is_none();
            // A disabled button still shows its tip (it senses hover only).
            let tip = if no_remote { "Add a remote first".to_owned() } else { hover(command, cx.remote.as_deref()) };
            let r = ui.add(buttons::secondary(command.name()).enabled(!busy && !no_remote)).on_hover_text(tip);
            if r.clicked() {
                msgs.push(Msg::Lfs(LfsMsg::Run { command, confirmed: false }));
            }
        }
    });
    if let Some(job) = &cx.job {
        ui.label(RichText::new(format!("{job}…")).size(type_scale::SMALL).color(color::weak()));
    }
}

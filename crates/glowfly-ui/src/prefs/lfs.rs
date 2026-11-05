//! Preferences > Git LFS (repository-specific): whether LFS is set up, the tracked patterns,
//! a field to track a new one, and the LFS commands. While git is held in the repository
//! (trust) the patterns read without git show, with Review trust…. A pattern from a nested
//! .gitattributes or info/attributes names its file and has no Untrack (git-lfs edits only the
//! root file). Every action is a `Msg`.

use super::{help, primary_button, row, text_field, warn};
use crate::theme::color;
use crate::theme::sizes::ControlSize;
use crate::theme::type_scale;
use crate::widgets::buttons;
use eframe::egui::{self, Key, RichText, Ui};
use glowfly_core::i18n::Lang;
use glowfly_core::lfs::{LfsCommand, LfsPattern, LfsStatus, PatternSource};
use glowfly_model::lfs::LfsMsg;
use glowfly_model::msg::FilesMsg;
use glowfly_model::msg::Msg;
use glowfly_model::msg::TrustMsg;
use glowfly_model::state::blocked::BUSY;
use glowfly_model::state::lfs::{INSTALL_URL, LfsPage};
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
    /// Every attributes file's patterns, read without git.
    pub patterns: Vec<LfsPattern>,
    pub lang: Lang,
}

/// `pattern` is the frame's copy of the new pattern field.
pub fn lfs_page(ui: &mut Ui, page: &LfsPage, pattern: &mut String, cx: &LfsContext, msgs: &mut Vec<Msg>) {
    let t = cx.lang;
    let Some(repo) = cx.repo_name.as_deref() else {
        help(ui, t.tr("lfs.page.no_repo.help"));
        return;
    };
    if let Some(real) = &cx.held {
        return held(ui, t, real, &cx.patterns, msgs);
    }
    if let Some(error) = &page.error {
        warn(ui, error);
        ui.add_space(6.0);
        if ui.add(buttons::secondary(t.tr("lfs.page.check_again.button"))).clicked() {
            msgs.push(Msg::Lfs(LfsMsg::Reload));
        }
        return;
    }
    let Some(status) = page.status.clone() else {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new());
            ui.label(RichText::new(t.tr("lfs.page.checking.body")).color(color::weak()));
        });
        return;
    };
    let busy = cx.job.is_some();
    match &status.version {
        None => missing(ui, t, &status, msgs),
        Some(_) if !status.initialized => not_set_up(ui, t, &status, repo, busy, msgs),
        Some(version) => set_up(ui, page, pattern, &status, version, cx, msgs),
    }
}

/// Git is held in the repository: nothing was read with it, and no command runs until it is
/// trusted. The patterns come from .gitattributes.
fn held(ui: &mut Ui, t: Lang, real: &Path, patterns: &[LfsPattern], msgs: &mut Vec<Msg>) {
    row(ui, t.tr("lfs.page.status.label"), |ui| {
        ui.label(RichText::new(t.tr("lfs.page.held.body")).color(color::text()));
        help(ui, t.tr("lfs.page.held.help"));
        ui.add_space(4.0);
        if ui.add(buttons::secondary(t.tr("lfs.page.review_trust.button"))).clicked() {
            msgs.push(Msg::Trust(TrustMsg::Ask { real: real.to_path_buf() }));
        }
    });
    row(ui, t.tr("lfs.page.patterns.label"), |ui| {
        if patterns.is_empty() {
            help(ui, t.tr("lfs.page.no_root_patterns.help"));
        } else {
            pattern_list(ui, t, patterns, false, true, msgs);
            help(ui, if all_root(patterns) { t.tr("lfs.page.read_from_root.help") } else { t.tr("lfs.page.read_from_files") });
        }
    });
}

/// git-lfs isn't installed: say so, and warn when the repository already uses LFS.
fn missing(ui: &mut Ui, t: Lang, status: &LfsStatus, msgs: &mut Vec<Msg>) {
    row(ui, t.tr("lfs.page.status.label"), |ui| {
        warn(ui, t.tr("lfs.page.missing.warn"));
        help(ui, t.tr("lfs.page.missing.help"));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.add(buttons::secondary(t.tr("lfs.page.get.button"))).clicked() {
                msgs.push(Msg::OpenUrl(INSTALL_URL.into()));
            }
            if ui.add(buttons::secondary(t.tr("lfs.page.check_again.button"))).clicked() {
                msgs.push(Msg::Lfs(LfsMsg::Reload));
            }
        });
    });
    if !status.patterns.is_empty() {
        row(ui, t.tr("lfs.page.patterns.label"), |ui| {
            warn(ui, &t.trn("lfs.page.missing.patterns.warn", status.patterns.len(), &[]));
            ui.add_space(4.0);
            pattern_list(ui, t, &status.patterns, false, true, msgs);
        });
    }
}

/// git-lfs is installed but this repository isn't set up.
fn not_set_up(ui: &mut Ui, t: Lang, status: &LfsStatus, repo: &str, busy: bool, msgs: &mut Vec<Msg>) {
    row(ui, t.tr("lfs.page.status.label"), |ui| {
        ui.label(RichText::new(t.trf("lfs.page.not_set_up.body", &[("repo", &repo)])).color(color::text()));
        help(ui, t.tr("lfs.page.not_set_up.help"));
        if !status.foreign_hooks.is_empty() {
            warn(ui, &t.trf("lfs.page.foreign_hooks.warn", &[("hooks", &status.foreign_hooks.join(", "))]));
        }
        ui.add_space(4.0);
        if primary_button(ui, t.tr("lfs.page.initialize.button"), !busy, busy.then_some(BUSY)).clicked() {
            msgs.push(Msg::Lfs(LfsMsg::Initialize));
        }
    });
    if !status.patterns.is_empty() {
        row(ui, t.tr("lfs.page.patterns.label"), |ui| {
            pattern_list(ui, t, &status.patterns, false, true, msgs);
            help(ui, t.tr("lfs.page.initialize.help"));
        });
    }
}

/// The repository is set up: patterns, the new-pattern field, .gitattributes and commands.
fn set_up(ui: &mut Ui, page: &LfsPage, pattern: &mut String, status: &LfsStatus, version: &str, cx: &LfsContext, msgs: &mut Vec<Msg>) {
    let (t, busy) = (cx.lang, cx.job.is_some());
    row(ui, t.tr("lfs.page.status.label"), |ui| {
        ui.label(RichText::new(t.trf("lfs.page.enabled.body", &[("version", &version)])).color(color::text()));
    });
    row(ui, t.tr("lfs.page.patterns.label"), |ui| {
        if status.patterns.is_empty() {
            help(ui, t.tr("lfs.page.no_patterns.help"));
        } else {
            pattern_list(ui, t, &status.patterns, true, busy, msgs);
        }
        help(ui, t.tr("lfs.page.patterns.help"));
    });
    row(ui, t.tr("lfs.page.new_pattern.label"), |ui| {
        let mut submit = false;
        ui.horizontal(|ui| {
            // Always enabled: disabling it while a job (an auto-fetch, say) runs would take the
            // focus away mid-typing. Only Track waits for the job.
            // A change clears the pattern's error (`PrefsState::apply`).
            let r = text_field(ui, pattern, t.tr("lfs.page.new_pattern.hint"));
            submit = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            submit |= ui.add(buttons::secondary(t.tr("lfs.page.track.button")).enabled(!busy)).clicked();
        });
        if submit && !busy {
            msgs.push(Msg::Lfs(LfsMsg::Track(pattern.clone())));
        }
        if let Some(e) = &page.pattern_error {
            warn(ui, e);
        }
    });
    row(ui, ".gitattributes", |ui| {
        if ui.add(buttons::secondary(t.tr("lfs.page.open_gitattributes.button")).enabled(status.has_gitattributes)).clicked() {
            msgs.push(Msg::Files(FilesMsg::OpenWorkingFile(".gitattributes".into())));
        }
        if !status.has_gitattributes {
            help(ui, t.tr("lfs.page.gitattributes.help"));
        }
    });
    row(ui, t.tr("lfs.page.commands.label"), |ui| commands_row(ui, cx, msgs));
}

fn all_root(patterns: &[LfsPattern]) -> bool {
    patterns.iter().all(|p| p.source.is_root())
}

/// Where a pattern from another file than the root .gitattributes applies.
fn source_tip(t: Lang, source: &PatternSource) -> String {
    match source {
        PatternSource::Nested(folder) => t.trf("lfs.page.source.nested.tip", &[("file", &source.file()), ("folder", folder)]),
        _ => t.tr("lfs.page.source.info.tip").to_owned(),
    }
}

/// One line per pattern; `editable` adds an Untrack button (disabled while `busy`) to the root
/// file's. When any pattern comes from another file, each line names its file.
fn pattern_list(ui: &mut Ui, t: Lang, patterns: &[LfsPattern], editable: bool, busy: bool, msgs: &mut Vec<Msg>) {
    let name_files = !all_root(patterns);
    for p in patterns {
        ui.horizontal(|ui| {
            ui.set_min_height(22.0);
            ui.label(RichText::new(&p.display).monospace().color(color::text()));
            if p.lockable {
                ui.label(RichText::new(t.tr("lfs.page.lockable")).size(type_scale::CAPTION).color(color::faint()));
            }
            if name_files {
                let file = ui.label(RichText::new(p.source.file()).size(type_scale::CAPTION).color(color::weak()));
                if !p.source.is_root() {
                    file.on_hover_text(source_tip(t, &p.source));
                }
            }
            if editable && p.source.is_root() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let untrack = buttons::secondary(t.tr("lfs.page.untrack.button")).size(ControlSize::Small).enabled(!busy);
                    if ui.add(untrack).clicked() {
                        msgs.push(Msg::Lfs(LfsMsg::Untrack(p.raw.clone())));
                    }
                });
            }
        });
    }
}

fn hover(t: Lang, command: LfsCommand, remote: Option<&str>) -> String {
    match (command, remote) {
        (LfsCommand::Fetch, _) => t.tr("lfs.page.fetch.tip").to_owned(),
        (LfsCommand::Pull, _) => t.tr("lfs.page.pull.tip").to_owned(),
        (LfsCommand::Push, Some(remote)) => t.trf("lfs.page.push.tip", &[("remote", &remote)]),
        (LfsCommand::Push, None) => t.tr("lfs.page.push.any_remote.tip").to_owned(),
        (LfsCommand::Prune, _) => t.tr("lfs.page.prune.tip").to_owned(),
        (LfsCommand::Checkout, _) => t.tr("lfs.page.checkout.tip").to_owned(),
    }
}

fn commands_row(ui: &mut Ui, cx: &LfsContext, msgs: &mut Vec<Msg>) {
    let (t, busy) = (cx.lang, cx.job.is_some());
    ui.horizontal_wrapped(|ui| {
        for command in LfsCommand::ALL {
            let no_remote = command.needs_remote() && cx.remote.is_none();
            // A disabled button still shows its tip (it senses hover only).
            let tip = if no_remote { t.tr("lfs.page.no_remote.tip").to_owned() } else { hover(t, command, cx.remote.as_deref()) };
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

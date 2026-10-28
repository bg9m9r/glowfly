//! The question before quitting while work runs (update/app.rs asks it): a git job that isn't
//! quiet (auto-fetch never asks; a sparse clone is such a job), a fetch or pull across
//! repositories, an update's download, or a terminal running a program (state/terminal/busy.rs).
//! Quitting can cut such work off partway, and Glowfly wouldn't see how it ends.

use super::menu_bar::tab_name;
use super::repo_sync::SyncKind;
use super::terminal::busy::{BusyTerminal, busy_lines, stops_text};
use super::updates::UpdateStatus;
use super::{AppState, Modal};
use crate::msg::{AppMsg, Msg};

/// The work that makes quitting ask.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Running {
    /// A git job, by its label ("Push"), in the repository named `repo` (its tab's name, or
    /// its folder's when no tab shows it); `None` for a job outside any repository (a clone).
    Job { label: String, repo: Option<String> },
    /// A fetch or pull across repositories, `done` of `all` finished.
    Sync { kind: SyncKind, done: usize, all: usize },
    /// Downloading the update to `version`.
    Download { version: String },
}

/// What the open quit question is about (`ViewState::quit_ask`): whether it named running work,
/// and the terminals it named. Once all of it has ended the window closes by itself
/// (update/app.rs `settle`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuitAsk {
    pub work: bool,
    pub terminals: Vec<BusyTerminal>,
}

/// What is running that quitting would cut off: the job first, then a repository sync, then a
/// download.
pub fn running(st: &AppState) -> Option<Running> {
    if let Some(job) = st.job.as_ref().filter(|j| !j.quiet) {
        let tab = job.workdir.as_deref().and_then(|w| st.tabs.iter().position(|t| t.path.as_deref() == Some(w)));
        let repo = match tab {
            Some(i) => Some(tab_name(st, i).to_owned()),
            None => job.workdir.as_deref().and_then(|w| w.file_name()).map(|n| n.to_string_lossy().into_owned()),
        };
        return Some(Running::Job { label: job.label.clone(), repo });
    }
    if let Some(sync) = st.repo_sync.as_ref().filter(|s| s.running()) {
        let (done, all) = sync.progress();
        return Some(Running::Sync { kind: sync.kind, done, all });
    }
    match &st.updates.status {
        UpdateStatus::Downloading(offer) => Some(Running::Download { version: offer.version.to_string() }),
        _ => None,
    }
}

/// The quit question while work runs or any of `busy` runs a program (Quit anyway sends
/// `AppMsg::QuitConfirmed`); `None` when nothing would be cut off.
pub fn quit_confirm(st: &AppState, busy: &[BusyTerminal]) -> Option<Modal> {
    let t = st.settings.lang();
    let work = running(st);
    if work.is_none() && busy.is_empty() {
        return None;
    }
    let mut lines: Vec<String> = work
        .iter()
        .map(|w| match w {
            Running::Job { label, repo: Some(repo) } => t.trf("quit.confirm.job", &[("job", label), ("repo", repo)]),
            Running::Job { label, repo: None } => t.trf("quit.confirm.job_no_repo", &[("job", label)]),
            Running::Sync { kind: SyncKind::Fetch, done, all } => t.trf("quit.confirm.sync_fetch", &[("done", done), ("all", all)]),
            Running::Sync { kind: SyncKind::Pull, done, all } => t.trf("quit.confirm.sync_pull", &[("done", done), ("all", all)]),
            Running::Download { version } => t.trf("quit.confirm.download", &[("version", version)]),
        })
        .collect();
    lines.extend(busy_lines(t, busy));
    let mut risks = Vec::new();
    if work.is_some() {
        risks.push(t.tr("quit.confirm.risk"));
    }
    if !busy.is_empty() {
        risks.push(stops_text(t, busy.len()));
    }
    let body = format!("{}\n\n{}", lines.join("\n"), risks.join(" "));
    Some(Modal::danger(t.tr("quit.confirm.title"), body, t.tr("quit.confirm.button"), Msg::App(AppMsg::QuitConfirmed)))
}

/// Whether the quit question is the open dialog.
pub fn asking(st: &AppState) -> bool {
    matches!(&st.modal, Some(Modal::ConfirmDanger { then, .. }) if matches!(**then, Msg::App(AppMsg::QuitConfirmed)))
}

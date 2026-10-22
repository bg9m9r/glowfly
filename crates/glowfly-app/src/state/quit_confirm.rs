//! The question before quitting while git work runs (update/app.rs asks it): a git job that
//! isn't quiet (auto-fetch never asks), or a fetch or pull across repositories still running.
//! Quitting can cut such work off partway, and Glowfly wouldn't see how it ends.

use super::menu_bar::tab_name;
use super::repo_sync::SyncKind;
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
}

/// What is running that quitting would cut off: the job first, then a repository sync.
pub fn running(st: &AppState) -> Option<Running> {
    if let Some(job) = st.job.as_ref().filter(|j| !j.quiet) {
        let tab = job.workdir.as_deref().and_then(|w| st.tabs.iter().position(|t| t.path.as_deref() == Some(w)));
        let repo = match tab {
            Some(i) => Some(tab_name(st, i).to_owned()),
            None => job.workdir.as_deref().and_then(|w| w.file_name()).map(|n| n.to_string_lossy().into_owned()),
        };
        return Some(Running::Job { label: job.label.clone(), repo });
    }
    let sync = st.repo_sync.as_ref().filter(|s| s.running())?;
    let (done, all) = sync.progress();
    Some(Running::Sync { kind: sync.kind, done, all })
}

/// The quit question while work runs (Quit anyway sends `AppMsg::QuitConfirmed`); `None` when
/// nothing would be cut off.
pub fn quit_confirm(st: &AppState) -> Option<Modal> {
    let t = st.settings.lang();
    let first = match running(st)? {
        Running::Job { label, repo: Some(repo) } => t.trf("quit.confirm.job", &[("job", &label), ("repo", &repo)]),
        Running::Job { label, repo: None } => t.trf("quit.confirm.job_no_repo", &[("job", &label)]),
        Running::Sync { kind, done, all } => {
            let key = match kind {
                SyncKind::Fetch => "quit.confirm.sync_fetch",
                SyncKind::Pull => "quit.confirm.sync_pull",
            };
            t.trf(key, &[("done", &done), ("all", &all)])
        }
    };
    let body = format!("{first}\n\n{}", t.tr("quit.confirm.risk"));
    Some(Modal::danger(t.tr("quit.confirm.title"), body, t.tr("quit.confirm.button"), Msg::App(AppMsg::QuitConfirmed)))
}

/// Whether the quit question is the open dialog.
pub fn asking(st: &AppState) -> bool {
    matches!(&st.modal, Some(Modal::ConfirmDanger { then, .. }) if matches!(**then, Msg::App(AppMsg::QuitConfirmed)))
}

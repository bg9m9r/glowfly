//! The Git LFS preferences page: reading the status, setting LFS up, tracking and untracking
//! patterns, and the LFS commands, each run as the active git job. Also whether git-lfs is
//! installed (checked at startup and by each status read) and the file menu's Track in Git LFS.

use super::ops::{JobSpec, start_job};
use crate::effect::Effect;
use crate::lfs::LfsMsg;
use crate::msg::Msg;
use crate::state::lfs::{INIT_LABEL, LfsInstall, lfs_remote};
use crate::state::{AppState, PrefPage};
use std::time::Instant;
use trellis_core::lfs::{self, LfsCommand, LfsStatus, TrackTarget};

const NOT_INSTALLED: &str = "Git LFS isn't installed.";

pub(super) fn handle(st: &mut AppState, m: LfsMsg, now: Instant, fx: &mut Vec<Effect>) {
    match m {
        LfsMsg::ShowPage => {
            if let Some(p) = st.prefs.as_mut() {
                p.page = PrefPage::RepoLfs;
                load(st, fx);
            }
        }
        LfsMsg::Reload => load(st, fx),
        LfsMsg::Initialize => initialize(st, fx),
        LfsMsg::InitializeWhenReady => {
            fx.extend(super::update(st, Msg::OpenPrefs(PrefPage::RepoLfs), now));
            if let Some(p) = st.prefs.as_mut() {
                p.lfs.init_when_ready = true;
            }
        }
        LfsMsg::Track(text) => track(st, &text, fx),
        LfsMsg::Untrack(raw) => {
            if refuse_without_lfs(st) {
                return;
            }
            let label = format!("Untrack {}", lfs::unescape_pattern(&raw));
            start_job(st, JobSpec::new(label, vec![lfs::untrack_args(&raw)]), fx);
        }
        LfsMsg::Run { command: LfsCommand::Prune, confirmed: false } => {
            let body = "Delete local copies of old LFS files that recent commits don't use? Only files the remote already has are \
                        deleted, and Fetch can download them again.";
            let then = Msg::Lfs(LfsMsg::Run { command: LfsCommand::Prune, confirmed: true });
            fx.extend(super::update(st, Msg::confirm("Prune LFS files", body.into(), "Prune", then), now));
        }
        LfsMsg::Run { command, .. } => run(st, command, fx),
        LfsMsg::Loaded { generation, result } => loaded(st, generation, result, now, fx),
        LfsMsg::Detect => {
            st.lfs_install = LfsInstall::Checking;
            fx.push(Effect::DetectLfs { cwd: st.any_dir() });
        }
        LfsMsg::Detected(v) => st.lfs_install = LfsInstall::from_version(v),
        LfsMsg::TrackPath(t) => track_path(st, t, fx),
    }
}

/// Reads the active repository's status again for the open page.
pub(super) fn load(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    let Some(p) = st.prefs.as_mut() else { return };
    p.lfs.generation += 1;
    p.lfs.status = None;
    p.lfs.error = None;
    fx.push(Effect::LoadLfs { workdir, generation: p.lfs.generation });
}

/// A job finished in the active tab: refresh the page if it is open. A successful Initialize
/// may lead to the pull offer once the new status arrives.
pub(super) fn job_finished(st: &mut AppState, label: &str, ok: bool, fx: &mut Vec<Effect>) {
    let Some(p) = st.prefs.as_mut().filter(|p| p.page == PrefPage::RepoLfs) else { return };
    if ok && label == INIT_LABEL {
        p.lfs.offer_pull = true;
    }
    load(st, fx);
}

fn status(st: &AppState) -> Option<&LfsStatus> {
    st.prefs.as_ref().and_then(|p| p.lfs.status.as_ref())
}

/// True (after a log line) when the status says git-lfs is missing.
fn refuse_without_lfs(st: &mut AppState) -> bool {
    let missing = status(st).is_some_and(|s| !s.installed());
    if missing {
        st.log(false, NOT_INSTALLED);
    }
    missing
}

fn initialize(st: &mut AppState, fx: &mut Vec<Effect>) {
    if refuse_without_lfs(st) {
        return;
    }
    start_job(st, JobSpec::new(INIT_LABEL.into(), vec![lfs::install_args()]), fx);
}

fn track(st: &mut AppState, text: &str, fx: &mut Vec<Effect>) {
    if refuse_without_lfs(st) {
        return;
    }
    let Some(p) = st.prefs.as_mut() else { return };
    let patterns = p.lfs.status.as_ref().map(|s| s.patterns.as_slice()).unwrap_or_default();
    match lfs::check_pattern(text, patterns) {
        Err(problem) => p.lfs.pattern_error = Some(problem.message()),
        Ok(pattern) => {
            p.lfs.new_pattern.clear();
            p.lfs.pattern_error = None;
            start_job(st, JobSpec::new(format!("Track {pattern}"), vec![lfs::track_args(&pattern)]), fx);
        }
    }
}

/// The file menu's Track in Git LFS, checked again: the menu may have been drawn from an older
/// state.
fn track_path(st: &mut AppState, t: TrackTarget, fx: &mut Vec<Effect>) {
    match &st.lfs_install {
        LfsInstall::Installed(_) => {}
        LfsInstall::Missing => return st.log(false, NOT_INSTALLED),
        LfsInstall::Unknown | LfsInstall::Checking => return st.log(false, "Still checking whether Git LFS is installed."),
    }
    let Some(snap) = &st.snap else { return };
    if let Err(p) = lfs::check_target(&t, &snap.lfs_patterns) {
        return st.log(false, p.message());
    }
    start_job(st, JobSpec::new(format!("Track {} in LFS", t.display()), vec![t.args()]), fx);
}

fn run(st: &mut AppState, command: LfsCommand, fx: &mut Vec<Effect>) {
    let remote = st.snap.as_ref().and_then(lfs_remote);
    if command.needs_remote() && remote.is_none() {
        st.log(false, "Add a remote first.");
        return;
    }
    start_job(st, JobSpec::new(command.label().into(), vec![command.args(remote.as_deref())]), fx);
}

fn loaded(st: &mut AppState, generation: u64, result: Result<LfsStatus, String>, now: Instant, fx: &mut Vec<Effect>) {
    let version = result.as_ref().ok().map(|s| s.version.clone());
    let Some(p) = st.prefs.as_mut() else { return };
    if p.lfs.generation != generation {
        return;
    }
    if let Some(v) = version {
        st.lfs_install = LfsInstall::from_version(v);
    }
    match result {
        Ok(s) => p.lfs.status = Some(s),
        Err(e) => {
            p.lfs.status = None;
            p.lfs.error = Some(e);
        }
    }
    let init = std::mem::take(&mut p.lfs.init_when_ready);
    let offer = std::mem::take(&mut p.lfs.offer_pull);
    let Some(s) = p.lfs.status.clone() else { return };
    if init {
        if s.initialized {
            st.toast(true, "Git LFS is already set up in this repository", now);
        } else if s.installed() {
            initialize(st, fx);
        }
    }
    let has_remote = st.snap.as_ref().and_then(lfs_remote).is_some();
    if offer && s.initialized && !s.patterns.is_empty() && has_remote {
        let body = "This repository already tracks files in LFS. Pull them now so the working tree has their contents instead of \
                    pointer files?";
        let then = Msg::Lfs(LfsMsg::Run { command: LfsCommand::Pull, confirmed: true });
        fx.extend(super::update(st, Msg::confirm("Download LFS files?", body.into(), "Pull LFS files", then), now));
    }
}

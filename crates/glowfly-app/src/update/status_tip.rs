//! The note after a slow whole status read, shown once per repository: what would speed git's
//! read up, with a button that turns the untracked cache on. Trellis never turns the file system
//! monitor on itself: that starts a daemon or needs a hook.

use super::jobs::{JobSpec, start_job};
use crate::effect::Effect;
use crate::msg::{LoadMsg, Msg};
use crate::state::AppState;
use crate::state::toasts::ToastAction;
use glowfly_core::repo::{self, SlowStatus, StatusSpeedups};
use std::time::{Duration, Instant};

/// After a whole status read: shows the note if `slow` says it was slow, Trellis may run git's
/// own status here, and the note hasn't been shown for this repository.
pub(super) fn check(st: &mut AppState, slow: Option<&SlowStatus>, now: Instant, fx: &mut Vec<Effect>) {
    let Some(slow) = slow else { return };
    let Some(snap) = st.snap.as_ref() else { return };
    if !st.runs_git_unasked() || st.settings.status_tip_shown_for(&snap.real_workdir) {
        return;
    }
    let (real, workdir) = (snap.real_workdir.clone(), snap.workdir.clone());
    let s = slow.speedups;
    let works = repo::untracked_cache_works(st.git.info.as_ref().and_then(|i| i.version));
    if repo::untracked_cache_helps(s) && works {
        let text = note_text(slow.took, s, true);
        let action = ToastAction { label: "Turn on".into(), msg: Box::new(Msg::Load(LoadMsg::SpeedUpStatus(s))), workdir: Some(workdir) };
        st.toast_action(true, text.clone(), action, now);
        st.log_quiet(true, text);
    } else if !s.fsmonitor {
        let text = note_text(slow.took, s, false);
        st.toast(true, text.clone(), now);
        st.log_quiet(true, text);
    }
    st.settings.mark_status_tip_shown(&real);
    fx.push(super::prefs::save_settings(st));
}

/// The note's "Turn on": a logged job that turns the untracked cache on for the active repository.
pub(super) fn speed_up(st: &mut AppState, speedups: StatusSpeedups, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.snap.as_ref().map(|s| s.workdir.clone()) else { return };
    if !repo::untracked_cache_helps(speedups) {
        return;
    }
    let spec = JobSpec { cwd: Some(workdir), ..JobSpec::new("Turn on the untracked cache".into(), repo::untracked_cache_cmds(speedups)) };
    start_job(st, spec, fx);
}

/// The note for a read that took `took`, offering the untracked cache (`cache`) or not, and
/// mentioning the file system monitor while it is off.
pub(super) fn note_text(took: Duration, s: StatusSpeedups, cache: bool) -> String {
    let mut text = format!("Reading this repository's changes took {:.1} s.", took.as_secs_f64());
    let monitor = "git's built-in file system monitor (core.fsmonitor)";
    if cache {
        text.push_str(" Git's untracked cache can speed this up");
        if s.show_untracked_all.is_none() {
            text.push_str("; turning it on also makes git status in a terminal list every untracked file rather than their folders");
        }
        text.push('.');
        if !s.fsmonitor {
            text.push_str(&format!(" On Windows and macOS, {monitor} can help further."));
        }
    } else if !s.fsmonitor {
        text.push_str(&format!(" On Windows and macOS, {monitor} can speed this up."));
    }
    text
}

#[cfg(test)]
#[path = "status_tip_tests.rs"]
mod tests;

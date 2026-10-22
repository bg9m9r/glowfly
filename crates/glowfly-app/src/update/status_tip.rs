//! The note after a slow whole status read, shown once per repository: what would speed git's
//! read up, with a Turn on button for git's untracked cache and, on Windows and macOS with git
//! 2.36 or newer, its built-in file system monitor: a job that starts `git fsmonitor--daemon`,
//! confirms it is watching and only then sets core.fsmonitor=true, saying in plain words why when
//! it can't run. On Linux the note suggests Watchman with git's fsmonitor-watchman hook; Glowfly
//! never sets core.fsmonitor to a hook, which the trust check would flag.

use super::jobs::{JobSpec, start_job};
use crate::effect::Effect;
use crate::msg::{LoadMsg, Msg};
use crate::state::AppState;
use crate::state::toasts::ToastAction;
use glowfly_core::git_version::Version;
use glowfly_core::opens::Os;
use glowfly_core::repo::{self, MonitorOffer, MonitorOutcome, SlowStatus, StatusSpeedups};
use std::time::{Duration, Instant};

/// What the note says about a file system monitor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MonitorNote {
    /// core.fsmonitor is already set: nothing to say.
    On,
    /// Git's built-in monitor, with a button.
    Offer,
    /// The repository is on a network drive, which the built-in monitor doesn't watch.
    Remote,
    /// This git is older than the built-in monitor.
    TooOld(Version),
    /// Linux: Watchman with git's hook, in words only.
    Watchman,
}

/// What the note says about the monitor, given the speed-ups set, what the OS and git offer and
/// whether the watcher found the repository on a network volume.
pub(super) fn monitor_note(s: StatusSpeedups, offer: MonitorOffer, remote: bool) -> MonitorNote {
    match offer {
        _ if s.fsmonitor => MonitorNote::On,
        MonitorOffer::Daemon if remote => MonitorNote::Remote,
        MonitorOffer::Daemon => MonitorNote::Offer,
        MonitorOffer::TooOld(v) => MonitorNote::TooOld(v),
        MonitorOffer::Watchman => MonitorNote::Watchman,
    }
}

/// After a whole status read: shows the note if `slow` says it was slow, Glowfly may run git's
/// own status here, and the note hasn't been shown for this repository.
pub(super) fn check(st: &mut AppState, slow: Option<&SlowStatus>, now: Instant, fx: &mut Vec<Effect>) {
    check_for(st, slow, Os::current(), now, fx)
}

/// `check` as on `os`.
pub(super) fn check_for(st: &mut AppState, slow: Option<&SlowStatus>, os: Os, now: Instant, fx: &mut Vec<Effect>) {
    let Some(slow) = slow else { return };
    let Some(snap) = st.snap.as_ref() else { return };
    if !st.runs_git_unasked() || st.settings.status_tip_shown_for(&snap.real_workdir) {
        return;
    }
    let (real, workdir) = (snap.real_workdir.clone(), snap.workdir.clone());
    let s = slow.speedups;
    let version = st.git.info.as_ref().and_then(|i| i.version);
    let remote = st.watch.remote.contains_key(&workdir);
    let note = monitor_note(s, repo::monitor_offer_for(os, version), remote);
    let cache = repo::untracked_cache_helps(s) && repo::untracked_cache_works(version);
    let text = note_text(slow.took, s, cache, &note);
    if cache || note == MonitorNote::Offer {
        let msg = Msg::Load(LoadMsg::SpeedUpStatus { speedups: s, cache, monitor: note == MonitorNote::Offer });
        let action = ToastAction { label: "Turn on".into(), msg: Box::new(msg), workdir: Some(workdir) };
        st.toast_action(true, text.clone(), action, now);
        st.log_quiet(true, text);
    } else if note != MonitorNote::On {
        st.toast(true, text.clone(), now);
        st.log_quiet(true, text);
    }
    st.settings.mark_status_tip_shown(&real);
    fx.push(super::prefs::save_settings(st));
}

/// The note for a read that took `took`, offering the untracked cache (`cache`) or not, and
/// saying `monitor` about the file system monitor.
pub(super) fn note_text(took: Duration, s: StatusSpeedups, cache: bool, monitor: &MonitorNote) -> String {
    let mut text = format!("Reading this repository's changes took {:.1} s.", took.as_secs_f64());
    let name = "file system monitor (core.fsmonitor)";
    let terminal = s.show_untracked_all.is_none();
    if cache && *monitor == MonitorNote::Offer {
        text.push_str(&format!(
            " Git's untracked cache and its built-in {name} can speed this up; the monitor is a small git process that keeps running in \
             the background."
        ));
        if terminal {
            text.push_str(" Turning the cache on also makes git status in a terminal list every untracked file rather than their folders.");
        }
        return text;
    }
    let ends = if cache {
        text.push_str(" Git's untracked cache can speed this up");
        if terminal {
            text.push_str("; turning it on also makes git status in a terminal list every untracked file rather than their folders");
        }
        text.push('.');
        "can help further"
    } else {
        "can speed this up"
    };
    match monitor {
        MonitorNote::On => {}
        MonitorNote::Offer => {
            text.push_str(&format!(" Git's built-in {name} {ends}; it is a small git process that keeps running in the background."))
        }
        MonitorNote::Remote => text.push_str(&format!(" Git's built-in {name} doesn't watch repositories on network drives.")),
        MonitorNote::TooOld(v) => {
            text.push_str(&format!(" Git 2.36 or newer has a built-in {name} that {ends} (you have {}.{}.{}).", v.0, v.1, v.2))
        }
        MonitorNote::Watchman => text.push_str(&format!(" On Linux, Watchman with git's fsmonitor-watchman hook (core.fsmonitor) {ends}.")),
    }
    text
}

/// The note's "Turn on" for the active repository: what the note offered (`cache`, `monitor`),
/// with the speed-ups set when it showed. The cache alone is a plain git job; with the monitor
/// it is the monitor job (`Effect::TurnOnMonitor`), through the trust gate.
pub(super) fn speed_up(st: &mut AppState, speedups: StatusSpeedups, cache: bool, monitor: bool, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.snap.as_ref().map(|s| s.workdir.clone()) else { return };
    if !monitor {
        if cache && repo::untracked_cache_helps(speedups) {
            let cmds = repo::untracked_cache_cmds(speedups);
            start_job(st, JobSpec { cwd: Some(workdir), ..JobSpec::new("Turn on the untracked cache".into(), cmds) }, fx);
        }
        return;
    }
    let then = if cache && repo::untracked_cache_helps(speedups) { repo::untracked_cache_cmds(speedups) } else { Vec::new() };
    let label = if then.is_empty() { "Turn on the file system monitor" } else { "Turn on the untracked cache and file system monitor" };
    super::jobs::begin_job(st, label, true, move |cwd| Effect::TurnOnMonitor { cwd, then }, fx);
}

/// The monitor job ended: done only when the monitor is watching and the cache, if asked for,
/// is on too; the headline says what is on, or in plain words why the monitor isn't.
pub(super) fn sped_up(st: &mut AppState, output: String, monitor: MonitorOutcome, cache: Option<bool>, now: Instant, fx: &mut Vec<Effect>) {
    let ok = monitor.is_watching() && cache != Some(false);
    let headline = sped_up_headline(&monitor, cache);
    super::ops::finish_job(st, ok, output, headline, None, now, fx);
}

fn sped_up_headline(monitor: &MonitorOutcome, cache: Option<bool>) -> Option<String> {
    Some(match (monitor, cache) {
        (MonitorOutcome::Watching { .. }, Some(true)) => "Git's untracked cache and file system monitor are on.".into(),
        (MonitorOutcome::Watching { .. }, None) => "Git's file system monitor is on and watching this repository.".into(),
        (MonitorOutcome::Watching { .. }, Some(false)) => {
            "Git's file system monitor is on, but turning on the untracked cache failed.".into()
        }
        (MonitorOutcome::Trouble(t), Some(true)) => format!("The untracked cache is on. {}", t.text()),
        (MonitorOutcome::Trouble(t), _) => t.text(),
    })
}

#[cfg(test)]
#[path = "status_tip_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "status_tip_monitor_tests.rs"]
mod monitor_tests;

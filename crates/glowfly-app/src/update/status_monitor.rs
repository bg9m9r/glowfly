//! Turning git's file system monitor off from Repository > Hooks: a job, through the trust gate
//! like Turn on, that unsets core.fsmonitor and then stops the daemon (`Effect::TurnOffMonitor`),
//! and keeping the page's On or Off in step once either job ends. Turn on is the slow-status
//! note's job (status_tip.rs), which the page starts too.

use crate::effect::{Effect, MonitorOff};
use crate::state::AppState;
use std::time::Instant;

/// Repository > Hooks' Turn off, for the active repository.
pub(super) fn turn_off(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.snap.is_none() {
        return;
    }
    let label = st.settings.lang().tr("prefs.hooks.monitor.off.job");
    super::jobs::begin_job(st, label, true, |cwd| Effect::TurnOffMonitor(MonitorOff { cwd }), fx);
}

/// The Turn off job ended: says so, and the page shows the monitor off once it is.
pub(super) fn turned_off(st: &mut AppState, output: String, ok: bool, now: Instant, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    let headline = if ok { t.tr("prefs.hooks.monitor.off.done") } else { t.tr("prefs.hooks.monitor.off.failed") };
    if ok {
        shown_as(st, false);
    }
    super::ops::finish_job(st, ok, output, Some(headline.to_owned()), None, now, fx);
}

/// Keeps Repository > Hooks' file system monitor row as the job left it, in the loaded values
/// and in the copy the Save bar compares with, so it never counts as an edit.
pub(super) fn shown_as(st: &mut AppState, on: bool) {
    let Some(p) = st.prefs.as_mut() else { return };
    for values in [p.git.as_mut(), p.git_saved.as_mut()].into_iter().flatten() {
        values.fsmonitor = on;
    }
}

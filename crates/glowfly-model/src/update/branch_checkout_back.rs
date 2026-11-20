//! After a tracker's fast-forward-then-checkout (update/branch_checkout.rs) failed once the
//! branch had moved, a quiet job moves it back, and the log says where it ended up. The
//! checkout's own failure text stays as it is: it holds git's reason and its fix.

use super::jobs::{JobSpec, Started, start_job};
use crate::effect::Effect;
use crate::state::AppState;
use crate::state::blocked;
use crate::state::remote_branch::move_back::{FastForwardCheckout, FfPhase};
use std::path::PathBuf;
use std::time::Instant;

/// The label of the job that moves the branch back.
pub(super) const MOVE_BACK_LABEL: &str = "Move branch back";

/// Remembers `pending` while the job that `started` describes runs (also when it waits for the
/// trust prompt, as the pending tag checkout does).
pub(super) fn started(st: &mut AppState, started: Started, pending: FastForwardCheckout) {
    if started != Started::No {
        st.ff_checkout = Some(pending);
    }
}

/// What a job that just ended leads to.
pub(super) enum Then {
    /// The checkout failed after the branch moved: move it back, in the failed job's folder.
    MoveBack(FastForwardCheckout, Option<PathBuf>),
    /// The move-back ended: what to log, and a toast when it failed.
    Said { ok: bool, log: String, toast: Option<String> },
}

/// Called with the job still active: takes the pending record and says what the ended job leads
/// to. None for any other job, a success, or a failure before the fast-forward.
pub(super) fn ended(st: &mut AppState, ok: bool, output: &str) -> Option<Then> {
    let p = st.ff_checkout.take()?;
    let job = st.job.as_ref()?;
    let t = st.settings.lang();
    match p.phase {
        FfPhase::Checkout => {
            if job.label != "Checkout" || ok || !p.moved(output) {
                return None;
            }
            let workdir = job.workdir.clone();
            Some(Then::MoveBack(p, workdir))
        }
        FfPhase::MovingBack => {
            if job.label != MOVE_BACK_LABEL {
                return None;
            }
            if ok {
                return Some(Then::Said { ok: true, log: p.moved_back(t), toast: None });
            }
            let reason = super::job_errors::reason(t, output);
            Some(Then::Said { ok: false, log: p.not_moved_back(t, &reason), toast: Some(p.not_moved_back_toast(t)) })
        }
    }
}

/// Acts on `ended`'s answer once the ended job is finished: starts the move-back, or logs how
/// it went.
pub(super) fn then(st: &mut AppState, then: Option<Then>, now: Instant, fx: &mut Vec<Effect>) {
    match then {
        None => {}
        Some(Then::MoveBack(p, cwd)) => move_back(st, p, cwd, now, fx),
        Some(Then::Said { ok, log, toast }) => say(st, ok, log, toast, now),
    }
}

fn say(st: &mut AppState, ok: bool, log: String, toast: Option<String>, now: Instant) {
    st.log(ok, log);
    if let Some(toast) = toast {
        st.toast(false, toast, now);
    }
}

/// Starts the quiet job that moves the branch back to where the fast-forward found it, run only
/// while the branch still is at the fast-forward's commit.
fn move_back(st: &mut AppState, p: FastForwardCheckout, cwd: Option<PathBuf>, now: Instant, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    let fail = |st: &mut AppState, reason: &str| {
        let (log, toast) = (p.not_moved_back(t, reason), p.not_moved_back_toast(t));
        say(st, false, log, Some(toast), now);
    };
    if st.job.is_some() {
        return fail(st, blocked::busy(t));
    }
    let spec = JobSpec { quiet: true, cwd, expect: Some(p.expect()), ..JobSpec::new(MOVE_BACK_LABEL.into(), vec![p.move_back_cmd()]) };
    let last = st.log.last().map(|l| l.id);
    match start_job(st, spec, fx) {
        Started::No => {
            // The trust gate's refusal was logged by the launch.
            let said = st.log.last().filter(|l| Some(l.id) != last).map(|l| l.text.clone());
            let reason = said.unwrap_or_else(|| blocked::busy(t).to_owned());
            fail(st, &reason);
        }
        Started::Now | Started::Held => st.ff_checkout = Some(FastForwardCheckout { phase: FfPhase::MovingBack, ..p }),
    }
}

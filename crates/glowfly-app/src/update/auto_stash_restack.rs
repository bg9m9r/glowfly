//! The auto-stash of a restack: `glowfly_core::stack` makes it once before the first step and
//! puts it back itself on the branch checked out before, so it never goes through the gate or
//! `wrap`. While a stopped restack waits for the banner, its stash is a pending auto-stash
//! (`waits`), which the WIP banner names and the saved records keep for a restart; `held` keeps
//! the ordinary put-back away from it until the restack ends. `put_back` forgets it once the
//! restack tried, telling how that went as any auto-stash's put-back does; `release` leaves it
//! in the stash list, saying so, when the restack can't end.

use super::auto_stash_saved;
use crate::effect::{Effect, RestackJob};
use crate::state::AppState;
use crate::state::auto_stash::{AutoStashJob, PendingStash, Phase};
use git2::Oid;
use glowfly_core::paths::same_spelling;
use glowfly_core::stash_ops::{AutoStashFor, PutBack};
use std::path::Path;
use std::time::Instant;

/// The stopped restack `job` in `workdir` holds its stash: it waits as a pending auto-stash
/// (once), replacing one of an earlier operation there, which stays in the stash list.
pub(super) fn waits(st: &mut AppState, workdir: &Path, job: &RestackJob, fx: &mut Vec<Effect>) {
    let (Some(op), Some(oid)) = (&job.stash_op, job.plan.stash) else { return };
    if let Some(i) = st.auto_stash.index_of(workdir) {
        if st.auto_stash.pending[i].oid == Some(oid) {
            return;
        }
        let old = st.auto_stash.pending.remove(i);
        let t = st.settings.lang();
        st.log_quiet(true, t.trf("auto_stash.no_longer_waiting", &[("name", &old.job.message)]));
        if old.saved {
            auto_stash_saved::save(st, workdir, fx);
        }
    }
    let job = AutoStashJob { op: op.clone(), message: op.message(), before: Vec::new() };
    let phase = Phase::Ended { from: st.load_gen + 1 };
    st.auto_stash.pending.push(PendingStash { workdir: workdir.to_path_buf(), job, oid: Some(oid), phase, told: false, saved: false });
}

/// Whether the pending auto-stash `p` belongs to a restack of its repository that is still
/// stopped: the restack puts it back itself.
pub(super) fn held(st: &AppState, p: &PendingStash) -> bool {
    matches!(p.job.op, AutoStashFor::Restack { .. }) && st.restacks.iter().any(|r| same_spelling(&r.workdir, &p.workdir))
}

/// The restack in `workdir` tried to put its stash `oid` (made for `op`) back, with `outcome`
/// (`output` is the run's): its pending entry and saved record go, and a toast tells how it went.
#[allow(clippy::too_many_arguments)]
pub(super) fn put_back(
    st: &mut AppState,
    workdir: &Path,
    op: &AutoStashFor,
    oid: Oid,
    outcome: PutBack,
    output: &str,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    forget(st, workdir, |p| p.oid == Some(oid), fx);
    super::auto_stash::told(st, &op.message(), outcome, output, oid, workdir.to_path_buf(), now);
}

/// The stopped restack in `workdir` can't end: its stash (made for `op`) stays in the stash list
/// and is no longer waited for.
pub(super) fn release(st: &mut AppState, workdir: &Path, op: &AutoStashFor, fx: &mut Vec<Effect>) {
    forget(st, workdir, |p| p.job.op == *op, fx);
    let t = st.settings.lang();
    st.log(false, t.trf("stack.stash_left", &[("name", &op.message())]));
}

/// Drops `workdir`'s pending entry matching `which`, rewriting the saved records when it was in
/// them.
fn forget(st: &mut AppState, workdir: &Path, which: impl Fn(&PendingStash) -> bool, fx: &mut Vec<Effect>) {
    let Some(i) = st.auto_stash.pending.iter().position(|p| same_spelling(&p.workdir, workdir) && which(p)) else { return };
    if st.auto_stash.pending.remove(i).saved {
        auto_stash_saved::save(st, workdir, fx);
    }
}

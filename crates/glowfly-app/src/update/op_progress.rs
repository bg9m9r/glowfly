//! The operation in progress: the strip's click (the WIP row and the details panel) and the
//! banner's confirmed Abort, which keeps the work of a merge, single cherry-pick or single
//! revert first (resolutions, conflicts and the operation itself), so Undo brings it back.

use crate::effect::{Effect, StashEffect};
use crate::msg::JobMsg;
use crate::state::op_confirm::can_keep;
use crate::state::{AppState, Selection};
use std::time::Instant;

/// Selects the WIP row, shows the details panel and scrolls the graph to its top (where the WIP
/// row is); the central view stays as it is.
pub(super) fn show(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let wip = snap.graph.wip.is_some();
    super::layout::reveal_details(st, fx);
    if st.session.selection != Selection::Wip {
        super::repo::select_wip(st, fx);
    }
    if wip {
        st.view.scroll_to_row = Some(0);
    }
}

/// Runs the confirmed abort `args` (named `label`). With `keep`, its work is kept first and the
/// abort recorded for Undo (`StashEffect::AbortKeeping`), unless nothing is left to keep since
/// the dialog asked.
pub(super) fn abort(st: &mut AppState, label: String, args: Vec<String>, keep: bool, now: Instant, fx: &mut Vec<Effect>) {
    if !keep {
        super::ops::job(st, JobMsg::Git { label, cmds: vec![args] }, now, fx);
        return;
    }
    if !st.snap.as_ref().is_some_and(can_keep) {
        let t = st.settings.lang();
        st.toast(false, t.tr("op.abort.changed.toast"), now);
        return;
    }
    super::ops::begin_job(st, &label, true, |cwd| Effect::Stash(StashEffect::AbortKeeping { cwd }), fx);
}

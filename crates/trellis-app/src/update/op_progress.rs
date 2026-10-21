//! The operation in progress: the strip's click (the WIP row and the details panel) and the
//! banner's confirmed Abort, which keeps the resolved changes of a merge, single cherry-pick or
//! single revert in a stash so Undo brings them back.

use super::ops::{JobSpec, start_job};
use crate::effect::Effect;
use crate::msg::JobMsg;
use crate::state::op_confirm::can_keep;
use crate::state::{AppState, Selection};
use std::time::Instant;
use trellis_core::gitcli::args as cmd;
use trellis_core::undo::{self, ABORT_STASH_MESSAGE, UndoKind};

/// Selects the WIP row, shows the details panel and scrolls the graph to its top (where the WIP
/// row is); the central view stays as it is.
pub(super) fn show(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let wip = snap.graph.wip.is_some();
    super::layout::reveal_details(st, fx);
    if st.selection != Selection::Wip {
        super::repo::select_wip(st, fx);
    }
    if wip {
        st.view.scroll_to_row = Some(0);
    }
}

/// Runs the confirmed abort `args` (named `label`). With `keep`, its changes are stashed first,
/// recorded as an undoable stash push, unless conflicts or edits outside the operation's files
/// appeared since the dialog asked.
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
    let replay = cmd(&["stash", "push", "-m", ABORT_STASH_MESSAGE]);
    let spec =
        JobSpec { record: Some(UndoKind::StashPush { replay: Some(vec![replay]) }), ..JobSpec::new(label, undo::abort_keeping_commands()) };
    start_job(st, spec, fx);
}

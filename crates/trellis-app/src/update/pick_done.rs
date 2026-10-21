//! The end of a cherry-pick or revert job (`JobMsg::PickDone`): what the toast says when some or
//! all of the commits changed nothing and were skipped, and the log line naming them.

use crate::effect::{Effect, UndoResult};
use crate::state::AppState;
use crate::state::journal::short;
use std::time::Instant;
use trellis_core::i18n::Lang;
use trellis_core::pick_ops::{PickResult, Picking};

pub(super) fn finished(
    st: &mut AppState,
    ok: bool,
    output: String,
    undo: Option<UndoResult>,
    r: PickResult,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    let t = st.settings.lang();
    let headline = text(t, ok, &r);
    super::ops::finish_job(st, ok, output, headline, undo, now, fx);
    // Toasts fade; the log keeps every commit that was skipped.
    if !r.skipped.is_empty() {
        let shas = r.skipped.iter().map(|&o| short(o)).collect::<Vec<_>>().join(", ");
        st.log(true, t.trf("pick.empty.skipped_log", &[("shas", &shas)]));
    }
}

/// The toast for a run that skipped commits (or stopped at one it couldn't skip); `None` keeps
/// the usual one (a plain success, or a conflict's explanation).
pub(super) fn text(t: Lang, ok: bool, r: &PickResult) -> Option<String> {
    let revert = r.picking == Picking::Revert;
    if let Some(o) = r.left_empty {
        let sha = [("sha", &short(o) as &dyn std::fmt::Display)];
        return Some(if revert { t.trf("pick.empty.left.revert", &sha) } else { t.trf("pick.empty.left.cherry_pick", &sha) });
    }
    let first = short(*r.skipped.first()?);
    if !ok {
        return None;
    }
    if r.made() == 0 {
        let (n, sha) = (r.total, [("sha", &first as &dyn std::fmt::Display)]);
        return Some(if revert { t.trn("pick.empty.all.revert", n, &sha) } else { t.trn("pick.empty.all.cherry_pick", n, &sha) });
    }
    let n = r.skipped.len();
    let args: [(&str, &dyn std::fmt::Display); 3] = [("sha", &first), ("done", &r.made()), ("total", &r.total)];
    Some(if revert { t.trn("pick.empty.some.revert", n, &args) } else { t.trn("pick.empty.some.cherry_pick", n, &args) })
}

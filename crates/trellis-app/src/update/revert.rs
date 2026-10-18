//! Reverting part of a commit or comparison diff into the working tree: Revert Hunk, Revert
//! lines (the line selection) and Revert changes to this file (file-row menus), and their
//! results, which are recorded in the tab's undo history. While File history is open, a revert
//! of one of its entries under an older name acts on the file's current name. With whitespace
//! changes hidden, a hunk or lines revert only the changes shown. Unlike staging, none of these
//! bump `diff_gen`: a commit diff's hunks can't change, so the other buttons stay valid (the
//! status refresh that follows reloads the diff anyway).

use crate::effect::Effect;
use crate::msg::RevertWhat;
use crate::state::AppState;
use std::time::Instant;
use trellis_core::repo::{DiffTarget, RevertPart, RevertRequest, Reverted};
use trellis_core::undo::Action;

/// Logged when the diff a hunk or line index was taken from has changed.
const DIFF_CHANGED: &str = "The diff changed before the revert ran, so nothing was changed. Try again on the updated diff.";

/// Reverts hunk `index` of the open commit or comparison diff shown at `generation`.
pub(super) fn hunk(st: &mut AppState, index: usize, generation: u64, fx: &mut Vec<Effect>) {
    if st.diff.as_ref().is_none_or(|(target, _)| !target.reverts_hunks()) {
        return;
    }
    if st.hunk_ops_gen() != Some(generation) {
        st.log(false, DIFF_CHANGED);
        return;
    }
    // Always the normal hunks, also in the whole-file view: core counts those.
    let Some((target, diff)) = &st.diff else { return };
    let Some(expected) = diff.hunks.get(index).cloned() else { return };
    let target = target.clone();
    push(st, target, RevertPart::Hunk { index, expected }, fx);
}

/// Reverts the selected lines of the open commit or comparison diff, when the selection
/// belongs to it. The selection is cleared.
pub(super) fn lines(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some((target, _)) = &st.diff else { return };
    let sel = &st.view.diff_lines;
    if !target.reverts_hunks() || sel.is_empty() || sel.target.as_ref() != Some(target) {
        return;
    }
    if let Some(why) = st.line_ops_blocked() {
        st.log(false, why);
        return;
    }
    // Line indexes are only good for the diff on screen.
    if st.hunk_ops_gen().is_none() {
        st.log(false, DIFF_CHANGED);
        return;
    }
    let Some((target, diff)) = &st.diff else { return };
    let Some(expected) = diff.hunks.get(st.view.diff_lines.hunk).cloned() else { return };
    let target = target.clone();
    let sel = std::mem::take(&mut st.view.diff_lines);
    let part = RevertPart::Lines { hunk: sel.hunk, expected, lines: sel.lines.into_iter().collect() };
    push(st, target, part, fx);
}

/// Reverts the whole change of `target` (a commit or comparison) to its file. It doesn't depend
/// on the open diff, so there is nothing to check here but the target.
pub(super) fn file(st: &mut AppState, target: DiffTarget, fx: &mut Vec<Effect>) {
    if !target.reverts_hunks() {
        return;
    }
    push(st, target, RevertPart::File, fx);
}

fn push(st: &AppState, target: DiffTarget, part: RevertPart, fx: &mut Vec<Effect>) {
    let Some(workdir) = st.workdir() else { return };
    let dest = dest(st, &target);
    // Toggling the whitespace filter reloads the diff (and bumps `diff_gen`), so the indexes of
    // a hunk or lines checked against `hunk_ops_gen` count the diff with this setting.
    let ignore_whitespace = st.settings.diff_ignore_whitespace && !matches!(part, RevertPart::File);
    fx.push(Effect::Revert { workdir, req: RevertRequest { target, part, dest, ignore_whitespace } });
}

/// The file's current name when `target` is a File history entry under an older one.
fn dest(st: &AppState, target: &DiffTarget) -> Option<String> {
    st.history.as_ref().and_then(|v| v.revert_dest(target))
}

/// A revert finished: its undo recorded, a toast for what changed (not ok when conflict markers
/// were left, which also goes to the log quietly), the log for a failure, then a status refresh.
pub(super) fn done(
    st: &mut AppState,
    path: String,
    what: RevertWhat,
    result: Result<Reverted, String>,
    undo: Option<Action>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    if let Some(a) = undo {
        st.undo.record(a);
    }
    match result {
        Ok(r @ Reverted::Conflicts) => {
            let text = done_text(&path, what, r);
            st.toast(false, text.clone(), now);
            st.log_quiet(false, text);
        }
        Ok(r) => st.toast(true, done_text(&path, what, r), now),
        Err(e) => st.log(false, fail_text(&path, what, &e)),
    }
    super::watch::settled(st, now);
    super::repo::refresh_status(st, fx);
}

/// What was reverted, e.g. "the hunk in a.txt".
fn reverted(path: &str, what: RevertWhat) -> String {
    match what {
        RevertWhat::Hunk => format!("the hunk in {path}"),
        RevertWhat::Lines(1) => format!("1 line in {path}"),
        RevertWhat::Lines(n) => format!("{n} lines in {path}"),
        RevertWhat::File => format!("the changes to {path}"),
    }
}

/// The toast (or, for conflicts, the log line) of a revert that changed `path`.
pub(crate) fn done_text(path: &str, what: RevertWhat, r: Reverted) -> String {
    let done = reverted(path, what);
    match r {
        Reverted::Applied => format!("Reverted {done}. The change is in your working tree."),
        Reverted::Merged => format!("Reverted {done}, merged with your later edits."),
        Reverted::Conflicts => format!("Reverted {done} with conflicts. Edit the file to resolve the conflict markers."),
        Reverted::Removed => format!("Removed {path}. The commit added it, and its contents are still in that commit."),
    }
}

/// The log line of a revert that failed.
pub(crate) fn fail_text(path: &str, what: RevertWhat, e: &str) -> String {
    match what {
        RevertWhat::Hunk => format!("Could not revert the hunk in {path}: {e}"),
        RevertWhat::Lines(_) => format!("Could not revert the selected lines in {path}: {e}"),
        RevertWhat::File => format!("Could not revert {path}: {e}"),
    }
}

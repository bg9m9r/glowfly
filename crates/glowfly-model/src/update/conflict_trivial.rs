//! Take the trivial picks in all files: one background op for the conflicted files that can be
//! given to git (those open with unsaved choices or in the merge tool are held back and named in
//! the log), and its result: the undo entry for the files it resolved, one toast with the counts
//! and an Undo button, one log line naming the skipped files, and the conflict view moving on
//! when its file was resolved.

use crate::effect::{Effect, LocalOp, UndoOpts};
use crate::msg::{JobMsg, Msg};
use crate::state::AppState;
use crate::state::conflict_files::trivial::{self, held_words, remaining, request, skip_words, skipped_text, summary};
use crate::state::toasts::ToastAction;
use glowfly_core::repo::TrivialTaken;
use glowfly_core::undo::Action;
use std::time::Instant;

/// The action was chosen: sent for the usable, not held files, or a toast saying why it can't be.
pub(super) fn take(st: &mut AppState, now: Instant, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    if let Some(why) = trivial::blocked(st, t) {
        st.toast(false, why, now);
        return;
    }
    let req = request(st);
    if !req.held.is_empty() {
        let held: Vec<(String, String)> = req.held.iter().map(|(p, h)| (p.clone(), held_words(t, h))).collect();
        st.log(true, skipped_text(t, &held));
    }
    let _ = super::ops::local(st, LocalOp::TakeTrivialPicks(req.paths), fx);
}

/// The op ended: its undo recorded, the skipped files logged, the open file moved on when it was
/// resolved, and one toast.
pub(super) fn taken(st: &mut AppState, taken: TrivialTaken, undo: Option<Action>, now: Instant, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    if let Some(a) = &undo {
        st.session.undo.record(a.clone());
    }
    if !taken.skipped.is_empty() {
        let skipped: Vec<(String, String)> = taken.skipped.iter().map(|(p, s)| (p.clone(), skip_words(t, s))).collect();
        st.log(true, skipped_text(t, &skipped));
    }
    let open = st.session.conflict.view.as_ref().map(|c| c.path.clone()).filter(|p| taken.resolved.contains(p));
    if let Some(path) = open {
        match st.snap.as_ref().and_then(|s| trivial::next_after(s, &taken.resolved, &path)) {
            Some(next) => super::conflict::open(st, next, fx),
            None => super::conflict::close(st),
        }
    }
    let left = st.snap.as_ref().map_or(0, |s| remaining(s, &taken.resolved));
    let nothing = taken.resolved.is_empty() && taken.left.is_empty() && taken.skipped.is_empty() && left == 0;
    if !nothing {
        let text = summary(t, taken.resolved.len(), left, !taken.skipped.is_empty());
        match undo.filter(|_| !taken.resolved.is_empty()) {
            Some(action) => {
                let label = t.tr("menu.edit.undo").into();
                let msg = Box::new(Msg::Job(JobMsg::UndoWith { redo: false, action: Box::new(action), opts: UndoOpts::default() }));
                st.toast_action(true, text, ToastAction { label, msg, workdir: st.workdir() }, now);
            }
            None => st.toast(!taken.resolved.is_empty(), text, now),
        }
    }
    super::local_done::settle(st, false, false, now, fx);
}

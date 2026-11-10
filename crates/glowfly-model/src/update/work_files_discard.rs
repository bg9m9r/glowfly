//! Discarding several Unstaged Files rows at once (the picked files): one danger confirmation
//! naming the count (and how the untracked ones go), the run it agreed to, and the result: one
//! undo entry for every file discarded, a toast, or a log line naming the file it stopped at.
//! A pick Glowfly can't act on safely blocks the whole discard; a single file left takes the
//! ordinary single-file flow. The trash-or-permanent choice is fixed when the confirmation
//! opens and travels inside it.

use crate::effect::{DeleteMode, DiscardEach, Effect, FilesEffect};
use crate::keymap::BindingId;
use crate::msg::{Msg, WorkFileMsg};
use crate::state::files::{TRASH, discard_msg, discard_unsafe};
use crate::state::path_hazards::hazard_of;
use crate::state::wip_lists::files;
use crate::state::{AppState, Modal};
use glowfly_core::error::WriteFailed;
use glowfly_core::i18n::Lang;
use glowfly_core::undo::Action;
use std::time::Instant;

/// Opens the confirmation for discarding `paths`, the ones still in Unstaged Files (in the
/// order given, which is the list's), or refuses when one of them has a hazard.
pub(super) fn ask(st: &mut AppState, paths: Vec<String>, now: Instant, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    let Some(snap) = &st.snap else { return };
    let listed = files(snap, false);
    let rows: Vec<_> = paths.iter().filter_map(|p| listed.iter().find(|f| f.path == *p).copied()).collect();
    if let Some(f) = rows.iter().find(|f| hazard_of(st, &f.path).is_some()) {
        let reason = hazard_of(st, &f.path).map(|h| h.reason(t)).unwrap_or_default();
        return st.log(false, discard_unsafe(t, &f.path, &reason));
    }
    match rows.as_slice() {
        [] => {}
        [one] => {
            let msg = discard_msg(t, one);
            fx.extend(super::update_sealed(st, msg, now));
        }
        _ => {
            let mode = DeleteMode::for_settings(&st.settings);
            let files: Vec<DiscardEach> = rows.iter().map(|f| DiscardEach { path: f.path.clone(), untracked: f.status == '?' }).collect();
            let hint = st.keymap.paren(BindingId::Undo);
            st.modal = Some(confirmation(t, files, mode, &hint));
        }
    }
}

/// The danger confirmation for discarding `files` the way `mode` says; `hint` is the Undo key.
fn confirmation(t: Lang, files: Vec<DiscardEach>, mode: DeleteMode, hint: &str) -> Modal {
    let n = files.len();
    let untracked = files.iter().filter(|f| f.untracked).count();
    let mut body = t.trn("details.wip.discard_files.body", n, &[("hint", &hint)]);
    if untracked > 0 {
        let note = if mode == DeleteMode::Trash {
            t.trn("details.wip.discard_files.untracked_trash", untracked, &[("trash", &TRASH)])
        } else {
            t.trn("details.wip.discard_files.untracked_permanent", untracked, &[("hint", &hint)])
        };
        body.push(' ');
        body.push_str(&note);
    }
    let title = t.trn("details.wip.discard_files.title", n, &[]);
    let button = t.trn("details.wip.discard_files.button", n, &[]);
    Modal::danger(title, body, button, Msg::WorkFile(WorkFileMsg::DiscardPathsConfirmed { files, mode }))
}

/// Runs the discard the confirmation agreed to; the picks end here.
pub(super) fn confirmed(st: &mut AppState, files: Vec<DiscardEach>, mode: DeleteMode, now: Instant, fx: &mut Vec<Effect>) {
    // `git restore` writes the tracked files, so filters and attributes apply.
    if files.iter().any(|f| !f.untracked) && !super::git_env::allows(st, glowfly_core::git_version::Feature::Restore, now) {
        return;
    }
    let Some(workdir) = st.workdir() else { return };
    st.cues.wip_picks = None;
    let _ = super::file_trust::push(st, FilesEffect::DiscardPaths { workdir, files, mode }, fx);
}

/// The runner discarded `done` files, then stopped at `failed` (if any): records the one undo
/// entry, says how it went and reads the status again.
pub(super) fn done(
    st: &mut AppState,
    done: usize,
    failed: Option<(String, WriteFailed)>,
    undo: Option<Action>,
    now: Instant,
    fx: &mut Vec<Effect>,
) {
    if let Some(a) = undo {
        st.session.undo.record(a);
    }
    let t = st.settings.lang();
    let hint = st.keymap.paren(BindingId::Undo);
    match failed {
        None => st.toast(true, t.trn("details.wip.discard_files.done", done, &[("hint", &hint)]), now),
        Some((path, e)) => {
            let why = crate::state::error_words::typed::write(t, &e);
            st.log(false, stopped_text(t, done, &path, &why, &hint));
        }
    }
    super::watch::settled(st, now);
    super::repo::refresh_status(st, fx);
}

/// The log line for a multi-file discard that discarded `done` files and then stopped at `path`
/// (`why`); `hint` is the Undo key, or "" for a tab that isn't active.
pub(super) fn stopped_text(t: Lang, done: usize, path: &str, why: &str, hint: &str) -> String {
    if done == 0 {
        return t.trf("details.wip.discard_files.failed", &[("path", &path), ("why", &why)]);
    }
    t.trn("details.wip.discard_files.partial", done, &[("path", &path), ("why", &why), ("hint", &hint)])
}

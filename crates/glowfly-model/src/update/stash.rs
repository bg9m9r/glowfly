//! Apply, pop and drop of one stash (sidebar STASHES menu and palette), and renaming one. A drop
//! is recorded for undo, which stores the dropped commit back in the list. The
//! stash list is numbered from the newest entry, so a stash made or dropped elsewhere since the
//! snapshot shifts every `stash@{N}`: the job names the entry by position for git but carries
//! the commit the user picked, and the runner refuses it if that position now holds another stash.
//! The question before applying or popping on top of uncommitted changes opens once the
//! stash's files are read. The Stash dialog's messages go to update/stash_push.rs.

use super::ops::{self, JobSpec, start_job};
use crate::effect::{Effect, StashEffect};
use crate::msg::{Msg, StashMsg};
use crate::state::palette::StashAction;
use crate::state::stash_confirm::{self, StashAsk};
use crate::state::stashes::RenameStashForm;
use crate::state::{AppState, Modal};
use git2::Oid;
use glowfly_core::gitcli::args;
use glowfly_core::repo::StashInfo;
use glowfly_core::stash_ops::edited_message;

const GONE: &str = "That stash no longer exists.";

pub(super) fn run(st: &mut AppState, action: StashAction, index: usize, oid: Oid, fx: &mut Vec<Effect>) {
    let refname = format!("stash@{{{index}}}");
    let cmd = args(&["stash", action.git_verb(), &refname]);
    // A drop is undoable while the tab is open; git's "Dropped stash@{N} (<sha>)" line also
    // lands in the log, so a dropped stash can be found again (`git stash apply <sha>`) until it
    // is garbage-collected.
    let spec = JobSpec { expect: Some((refname, oid)), ..JobSpec::new(action.job_label().into(), vec![cmd]) };
    start_job(st, spec, fx);
}

pub(super) fn handle(st: &mut AppState, msg: StashMsg, fx: &mut Vec<Effect>) {
    match msg {
        StashMsg::RenameDialog { oid } => match find(st, oid) {
            Some(s) => st.modal = Some(Modal::RenameStash(RenameStashForm::new(&s))),
            None => st.log(false, GONE),
        },
        StashMsg::Rename { oid, message } => rename(st, oid, &message, fx),
        StashMsg::PushDialog { scope, path } => super::stash_push::open(st, scope, path),
        StashMsg::Push { scope, message, paths, keep_index } => super::stash_push::push(st, scope, &message, paths, keep_index, fx),
        StashMsg::Ask(ask) => {
            if let Some(cwd) = st.workdir() {
                fx.push(Effect::Stash(StashEffect::ReadPaths { cwd, ask }));
            }
        }
        StashMsg::PathsRead { ask, paths } => open_question(st, &ask, paths),
    }
}

/// Opens the question before applying or popping on top of uncommitted changes, with the files
/// the stash and the working tree both change (the general warning when the stash's files
/// couldn't be read). Conflicts that appeared meanwhile refuse it instead; another dialog that
/// opened meanwhile keeps its place.
fn open_question(st: &mut AppState, ask: &StashAsk, paths: Result<Vec<String>, String>) {
    let t = st.settings.lang();
    let Some(snap) = st.snap.as_ref() else { return };
    if let Some(why) = stash_confirm::apply_blocked(snap, t) {
        st.log(false, why);
        return;
    }
    if st.modal.is_some() {
        tracing::debug!("a dialog opened while the stash's files were read; no question");
        return;
    }
    if let Err(e) = &paths {
        tracing::debug!("could not read the stash's files: {e}");
    }
    if let Msg::Confirm { title, body, button, then } = stash_confirm::ask_confirm(snap, t, ask, paths.as_deref().ok()) {
        st.modal = Some(Modal::Confirm { title, body, button, then });
    }
}

/// The stash with commit `oid` in the current snapshot.
fn find(st: &AppState, oid: Oid) -> Option<StashInfo> {
    st.snap.as_ref()?.stashes.iter().find(|s| s.oid == oid).cloned()
}

fn close_dialog(st: &mut AppState) {
    if matches!(st.modal, Some(Modal::RenameStash(_))) {
        st.modal = None;
    }
}

/// Starts renaming the stash with commit `oid` to the dialog's text `body`, found again by its
/// commit since the list may have shifted. The dialog stays open on a blank text or while
/// another job runs.
fn rename(st: &mut AppState, oid: Oid, body: &str, fx: &mut Vec<Effect>) {
    let Some(s) = find(st, oid) else {
        st.log(false, GONE);
        close_dialog(st);
        return;
    };
    let Some(message) = edited_message(&s.message, body) else {
        st.log(false, "Enter a message for the stash.");
        return;
    };
    if message == s.message {
        st.log(false, "That stash already has this message.");
        close_dialog(st);
        return;
    }
    if ops::busy(st, false) {
        return;
    }
    // Recorded undo actions find stashes by commit, which a rename keeps.
    let effect = move |cwd| Effect::Stash(StashEffect::Rename { cwd, index: s.index, oid, message });
    if ops::begin_job(st, "Rename stash", true, effect, fx) == ops::Started::Now {
        close_dialog(st);
    }
}

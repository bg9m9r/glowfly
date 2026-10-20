//! The Stash dialog (`Modal::StashPush`): opening it on a choice, and stashing every change,
//! only the staged changes or chosen files as one git job (`stash_ops::push_command`), so hooks,
//! config and Repository trust apply. `update` checks the choice against the snapshot again,
//! since the working tree may have changed after the dialog drew it.

use super::ops::{self, JobSpec, Started, start_job};
use crate::effect::Effect;
use crate::state::stashes::{StashPushForm, label, push_blocked, stashable_paths};
use crate::state::{AppState, Modal};
use trellis_core::stash_ops::{STAGED_MIN_GIT, StashScope, push_command, push_message, staged_unsupported};

/// Where a long selection's paths are written for `--pathspec-from-file`.
const PATHSPEC_FILE: &str = "TRELLIS_STASH_PATHS";

/// Opens the Stash dialog on `scope`, with only `path` ticked when given. Nothing opens on a
/// clean tree.
pub(super) fn open(st: &mut AppState, scope: StashScope, path: Option<String>) {
    let Some(snap) = &st.snap else { return };
    if !snap.is_dirty() {
        st.log(false, st.settings.lang().tr("stash.blocked.nothing"));
        return;
    }
    st.modal = Some(Modal::StashPush(StashPushForm::new(scope, path)));
}

fn close_dialog(st: &mut AppState) {
    if matches!(st.modal, Some(Modal::StashPush(_))) {
        st.modal = None;
    }
}

/// Stashes `scope` under the typed `body` (blank: git's own message). For Files only the
/// `paths` still changed in the snapshot and safe to name to git (`stashable`) count. A refused stash is logged and keeps the dialog
/// open, except on a clean tree; so does a busy one.
pub(super) fn push(st: &mut AppState, scope: StashScope, body: &str, paths: Vec<String>, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let paths: Vec<String> = if scope == StashScope::Files {
        let listed: std::collections::HashSet<String> = stashable_paths(snap).collect();
        paths.into_iter().filter(|p| listed.contains(p)).collect()
    } else {
        Vec::new()
    };
    if let Some(why) = push_blocked(st.settings.lang(), snap, scope, &paths) {
        let clean = !snap.is_dirty();
        st.log(false, why);
        if clean {
            close_dialog(st);
        }
        return;
    }
    let file = snap.git_dir.join(PATHSPEC_FILE);
    if ops::busy(st, false) {
        return;
    }
    let c = push_command(scope, push_message(body).as_deref(), &paths, &file);
    let spec = JobSpec { write_file: c.pathspec_file.map(|s| (file, s)), ..JobSpec::new(label(scope).into(), vec![c.cmd]) };
    if start_job(st, spec, fx) == Started::Now {
        close_dialog(st);
    }
}

/// The headline of the job that just ended, when it is a staged-only stash that git refused
/// for not knowing `--staged`.
pub(super) fn headline(st: &AppState, ok: bool, output: &str) -> Option<String> {
    let staged = st.job.as_ref().is_some_and(|j| j.label == label(StashScope::Staged));
    (!ok && staged && staged_unsupported(output)).then(|| {
        let (major, minor) = STAGED_MIN_GIT;
        format!("Stashing only staged changes needs git {major}.{minor} or newer. Nothing was stashed.")
    })
}

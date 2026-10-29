//! The Stash dialog (`Modal::StashPush`): opening it on a choice, and stashing every change,
//! only the staged changes or chosen files, optionally keeping the staged changes staged, as
//! one git job (`stash_ops::push_command`), so hooks, config and Repository trust apply. A
//! chosen staged rename is stashed whole: the job then also puts its old path back from HEAD
//! (`stash_ops::restore_commands`), never over a file that is there. `update` checks the
//! choice against the snapshot again, since the working tree may have changed after the
//! dialog drew it.

use super::ops::{self, JobSpec, Started, start_job};
use crate::effect::Effect;
use crate::state::stashes::{StashPushForm, label, push_blocked, stashable_paths, wip_files};
use crate::state::{AppState, Modal};
use glowfly_core::repo::{Snapshot, name_key};
use glowfly_core::stash_ops::{
    RESTORE_BLOCKED, STAGED_MIN_GIT, StashScope, push_command, push_message, rename_sources, restore_commands, staged_unsupported,
};
use glowfly_core::undo::UndoKind;
use std::collections::HashSet;

/// Where a long selection's paths are written for `--pathspec-from-file`.
const PATHSPEC_FILE: &str = "GLOWFLY_STASH_PATHS";

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

/// Stashes `scope` under the typed `body` (blank: git's own message), keeping the staged
/// changes staged too when `keep_index` (not for Staged). For Files only the `paths` still
/// changed in the snapshot and safe to name to git (`stashable`) count. A refused stash is
/// logged and keeps the dialog open, except on a clean tree; so does a busy one.
pub(super) fn push(st: &mut AppState, scope: StashScope, body: &str, paths: Vec<String>, keep_index: bool, fx: &mut Vec<Effect>) {
    let Some(snap) = &st.snap else { return };
    let paths: Vec<String> = if scope == StashScope::Files {
        let listed: HashSet<String> = stashable_paths(snap).collect();
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
    // Keeping the index keeps a staged rename whole by itself.
    let restore = if scope == StashScope::Files && !keep_index { renames_to_restore(snap, &paths) } else { Vec::new() };
    let file = snap.git_dir.join(PATHSPEC_FILE);
    if ops::busy(st, false) {
        return;
    }
    let c = push_command(scope, keep_index, push_message(body).as_deref(), &paths, &file);
    let mut cmds = vec![c.cmd];
    cmds.extend(restore_commands(&restore));
    // The follow-up commands are part of the stash: redo runs them again too.
    let record = match (cmds.len(), UndoKind::classify(&cmds[0])) {
        (1, _) => None,
        (_, Some(UndoKind::StashPush { replay })) => Some(UndoKind::StashPush { replay: replay.map(|_| cmds.clone()) }),
        (_, other) => other,
    };
    let spec = JobSpec { write_file: c.pathspec_file.map(|s| (file, s.into_bytes())), record, ..JobSpec::new(label(scope).into(), cmds) };
    if start_job(st, spec, fx) == Started::Now {
        close_dialog(st);
    }
}

/// The old paths of the staged renames among `paths` to put back from HEAD after the stash
/// (`rename_sources`). Not one whose name a folding volume sees as a working-tree row's
/// (`name_key`): the file there (a new untracked one, or the new name of a case-only rename)
/// stays and the rename's deletion stays staged. Something at an old path that the snapshot
/// doesn't show (an ignored file, one made since) stops the job before it stashes
/// (`stash_ops::restore_blocked`, checked by the runner on disk).
fn renames_to_restore(snap: &Snapshot, paths: &[String]) -> Vec<String> {
    let listed: HashSet<String> = wip_files(snap).iter().map(|f| name_key(&f.path)).collect();
    rename_sources(paths, &snap.staged).into_iter().filter(|p| !listed.contains(&name_key(p))).collect()
}

/// The headline of the job that just ended: a staged-only stash that git refused for not
/// knowing `--staged`, or a stash of chosen files that the runner stopped because something
/// sits at a staged rename's old path (its own English details, `RESTORE_BLOCKED`).
pub(super) fn headline(st: &AppState, ok: bool, output: &str) -> Option<String> {
    let job = st.job.as_ref().map(|j| j.label.as_str());
    if !ok && job == Some(label(StashScope::Files)) && output.starts_with(RESTORE_BLOCKED) {
        return Some(st.settings.lang().tr("stash.push.rename_blocked.toast").to_owned());
    }
    let staged = job == Some(label(StashScope::Staged));
    (!ok && staged && staged_unsupported(output)).then(|| {
        let (major, minor) = STAGED_MIN_GIT;
        format!("Stashing only staged changes needs git {major}.{minor} or newer. Nothing was stashed.")
    })
}

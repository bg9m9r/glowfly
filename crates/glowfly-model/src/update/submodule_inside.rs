//! Working inside a submodule from its parent's SUBMODULES row (state/submodules/inside.rs):
//! Record this checkout stages the gitlink through the usual Stage (git add); Fetch, Pull and
//! Discard changes inside run as jobs in the submodule's own folder, behind that repository's
//! trust gate. Pull reads the submodule's branch first and refuses a detached HEAD or a branch
//! without an upstream, offering to open the submodule; Discard asks first and runs Discard
//! all's stash command, recorded for undo in the submodule's own tab when it is open. When an
//! inside job ends, the parent refreshes if it is still the active tab.

use super::ops::{JobSpec, Started, start_job};
use super::submodules::{find, folder_of};
use crate::effect::{Effect, SubmoduleEffect};
use crate::msg::{Msg, StageMsg};
use crate::state::submodules::inside::{self, InsideJob, InsideKind};
use crate::state::{AppState, Modal, ToastAction};
use crate::submodule::SubmoduleMsg;
use glowfly_core::paths::same_spelling;
use glowfly_core::repo::InsideHead;
use glowfly_core::undo::UndoKind;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Record this checkout: stages the gitlink unless it can't be recorded (logged).
pub(super) fn record(st: &mut AppState, path: String, now: Instant, fx: &mut Vec<Effect>) {
    let Some(sm) = find(st, &path) else { return };
    let t = st.settings.lang();
    if let Some(why) = st.snap.as_ref().and_then(|s| inside::record_blocked(t, &sm, s)) {
        st.log(false, inside::refused(t, None, &path, why));
        return;
    }
    super::ops::stage(st, StageMsg::StageFile(sm.path), now, fx);
}

/// Fetch inside: a job in the submodule's folder.
pub(super) fn fetch(st: &mut AppState, path: String, fx: &mut Vec<Effect>) {
    start(st, InsideKind::Fetch, &path, fx);
}

/// Pull inside: reads the submodule's branch first (`head_read`).
pub(super) fn pull(st: &mut AppState, path: String, fx: &mut Vec<Effect>) {
    let Some(sm) = checked(st, InsideKind::Pull, &path) else { return };
    let Some(workdir) = st.workdir() else { return };
    fx.push(Effect::Submodule(SubmoduleEffect::ReadHead { workdir, path: sm.path }));
}

/// The submodule's branch, read for Pull: a detached HEAD or a branch without an upstream is
/// refused with a toast that opens the submodule; otherwise the pull runs.
pub(super) fn head_read(st: &mut AppState, path: String, result: Result<Option<InsideHead>, String>, now: Instant, fx: &mut Vec<Effect>) {
    let t = st.settings.lang();
    let head = match result {
        Ok(Some(h)) => h,
        Ok(None) => return st.log(false, t.trf("sidebar.submodules.inside.not_checked_out", &[("path", &path)])),
        Err(e) => return st.log(false, t.trf("sidebar.submodules.inside.head_failed", &[("path", &path), ("error", &e)])),
    };
    if let Some(text) = inside::pull_refused(t, &path, &head) {
        let action = open_action(st, &path);
        st.toast_action(false, text, action, now);
        return;
    }
    start(st, InsideKind::Pull, &path, fx);
}

/// Discard changes inside: asks first, then stashes everything inside the submodule.
pub(super) fn discard(st: &mut AppState, path: String, confirmed: bool, fx: &mut Vec<Effect>) {
    if confirmed {
        start(st, InsideKind::Discard, &path, fx);
        return;
    }
    let Some(sm) = checked(st, InsideKind::Discard, &path) else { return };
    let (title, body, button) = inside::discard_confirm(st.settings.lang(), &sm.path);
    let then = Msg::Submodule(SubmoduleMsg::DiscardInside { path: sm.path, confirmed: true });
    st.modal = Some(Modal::danger(title, body, button, then));
}

/// The submodule at `path`, when `kind` can run there; logs why not.
fn checked(st: &mut AppState, kind: InsideKind, path: &str) -> Option<glowfly_core::repo::SubmoduleInfo> {
    let sm = find(st, path)?;
    let t = st.settings.lang();
    if let Some(why) = inside::blocked(t, kind, &sm) {
        st.log(false, inside::refused(t, Some(kind), path, why));
        return None;
    }
    Some(sm)
}

/// Starts `kind`'s job in the folder of the submodule at `path`, and remembers it.
fn start(st: &mut AppState, kind: InsideKind, path: &str, fx: &mut Vec<Effect>) {
    let Some(sm) = checked(st, kind, path) else { return };
    let Some(parent) = st.workdir() else { return };
    let label = inside::label(kind, &sm.path);
    // A job with its own folder gets no undo record by itself: Discard is Discard all's.
    let record = (kind == InsideKind::Discard).then_some(UndoKind::DiscardAll);
    let Some(folder) = folder_of(&parent, &sm.path) else { return };
    let spec = JobSpec { cwd: Some(folder), record, ..JobSpec::new(label.clone(), inside::cmds(kind)) };
    if matches!(start_job(st, spec, fx), Started::Now | Started::Held) {
        st.submodules.inside = Some(InsideJob { parent, path: sm.path, kind, label });
    }
}

/// The toast button that opens the submodule at `path`, while its parent is the active tab.
fn open_action(st: &AppState, path: &str) -> ToastAction {
    ToastAction {
        label: st.settings.lang().tr("git_errors.open_submodule.button").to_owned(),
        msg: Box::new(Msg::Submodule(SubmoduleMsg::Open(path.to_owned()))),
        workdir: st.workdir(),
    }
}

/// The inside job remembered under `label`.
fn remembered<'a>(st: &'a AppState, label: &str) -> Option<&'a InsideJob> {
    st.submodules.inside.as_ref().filter(|j| j.label == label)
}

/// The active job's success headline when it is the remembered inside job (Discard names its
/// stash); None otherwise, and for a failure, which the failure toast explains.
pub(super) fn headline(st: &AppState, ok: bool) -> Option<String> {
    let job = st.job.as_ref()?;
    let inside = remembered(st, &job.label)?;
    inside::headline(st.settings.lang(), inside.kind, &inside.path, ok)
}

/// The active job labelled `label` ended: when it is the remembered inside job, the parent
/// refreshes if it is still the active tab (the submodule's row and gitlink may have changed),
/// and a successful Discard's toast offers to open the submodule.
pub(super) fn job_finished(st: &mut AppState, label: &str, ok: bool, fx: &mut Vec<Effect>) {
    if remembered(st, label).is_none() {
        return;
    }
    let Some(job) = st.submodules.inside.take() else { return };
    if !parent_active(st, &job.parent) {
        return;
    }
    super::repo::refresh(st, fx);
    if ok && job.kind == InsideKind::Discard {
        let action = open_action(st, &job.path);
        st.set_last_toast_action(action);
    }
}

fn parent_active(st: &AppState, parent: &Path) -> bool {
    st.workdir().is_some_and(|w: PathBuf| same_spelling(&w, parent))
}

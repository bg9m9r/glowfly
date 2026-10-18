//! Batch submodule updates: Initialize all and Update all from the header menu or the palette
//! (Update all asks before leaving commits behind), the offer to initialize submodules when a
//! repository opens, and keeping submodules updated after checkouts, pulls, merges, rebases and
//! resets (Preferences > General, overridden per repository from the header menu). That update
//! leaves out any submodule where it would leave commits behind, and says so in the log.

use super::ops::{JobSpec, Started, start_job};
use crate::effect::{Effect, LoadKind, SubmoduleEffect};
use crate::msg::Msg;
use crate::state::submodules::batch::{CONFIRM_ALL_BUTTON, confirm_all_body, job_label, nothing_to_do, offer_text, plan, skipped_line};
use crate::state::toasts::ToastAction;
use crate::state::trust::InheritKind;
use crate::state::{AppState, Modal};
use crate::submodule::{Batch, SubmoduleMsg};
use std::path::Path;
use std::time::Instant;
use trellis_core::paths::same_spelling;
use trellis_core::repo::SubmoduleInfo;
use trellis_core::settings::SubmoduleUpdates;

/// The header menu's or palette's batch: unconfirmed reads the submodules again first.
pub(super) fn batch(st: &mut AppState, batch: Batch, confirmed: bool, fx: &mut Vec<Effect>) {
    if confirmed {
        let list = st.snap.as_ref().map(|s| s.submodules.clone()).unwrap_or_default();
        run(st, batch, &list, true, fx);
    } else if let Some(workdir) = st.workdir() {
        fx.push(Effect::Submodule(SubmoduleEffect::CheckAll { workdir, batch }));
    }
}

/// The submodules read again for `batch`: the rows follow, then the batch runs (or asks).
pub(super) fn checked_all(st: &mut AppState, batch: Batch, result: Result<Vec<SubmoduleInfo>, String>, fx: &mut Vec<Effect>) {
    let list = match result {
        Ok(list) => list,
        Err(e) if batch == Batch::Auto => return st.log_quiet(false, format!("Could not read the submodules: {e}")),
        Err(e) => return st.log(false, format!("Could not read the submodules: {e}")),
    };
    if let Some(s) = st.snap.as_mut() {
        s.submodules = list.clone();
    }
    if batch == Batch::Auto {
        if !auto_wanted(st) || !st.runs_git_unasked() {
            return;
        }
        if st.job.is_some() {
            return st.log_quiet(true, "The submodules were not updated: another operation started first.");
        }
    }
    run(st, batch, &list, false, fx);
}

/// Plans `batch` over `list` and starts its job; Update all asks first (unless `confirmed`)
/// when it would leave commits behind.
fn run(st: &mut AppState, batch: Batch, list: &[SubmoduleInfo], confirmed: bool, fx: &mut Vec<Effect>) {
    let plan = plan(batch, list);
    for sm in &plan.skipped {
        st.log(true, skipped_line(sm));
    }
    if plan.targets.is_empty() {
        if batch != Batch::Auto {
            st.log(true, nothing_to_do(batch));
        }
        return;
    }
    if batch == Batch::UpdateAll && !plan.stranding.is_empty() && !confirmed {
        st.modal = Some(Modal::danger(
            "Update all submodules",
            confirm_all_body(&plan.stranding),
            CONFIRM_ALL_BUTTON,
            Msg::Submodule(SubmoduleMsg::Batch { batch: Batch::UpdateAll, confirmed: true }),
        ));
        return;
    }
    let cmds = crate::state::submodules::batch::cmds(batch, &plan);
    let label = job_label(batch);
    // The ones not checked out yet are trusted once they are, when the repository is.
    let new: Vec<&str> =
        list.iter().filter(|sm| !sm.is_initialized() && plan.targets.contains(&sm.path)).map(|sm| sm.path.as_str()).collect();
    if start_job(st, JobSpec::new(label.to_owned(), cmds), fx) == Started::Now
        && !new.is_empty()
        && let Some(workdir) = st.workdir()
    {
        let folders = new.iter().map(|p| super::submodules::folder_of(&workdir, p)).collect();
        super::trust_inherit::expect(st, label, InheritKind::Submodule, folders);
    }
}

/// Whether the active repository has submodules and keeps them updated after checkouts.
fn auto_wanted(st: &AppState) -> bool {
    st.snap.as_ref().is_some_and(|s| !s.submodules.is_empty() && st.settings.keeps_submodules_updated(&s.real_workdir))
}

/// A job labelled `label` started (or waits for the trust prompt) in the active tab: remembers
/// it when `moves` (it may change the commits the submodules are recorded at).
pub(super) fn job_started(st: &mut AppState, label: &str, moves: bool) {
    st.submodules.after_job = if moves { st.workdir().map(|w| (w, label.to_owned())) } else { None };
}

/// The active job ended. When it is the one `job_started` remembered, it succeeded in the tab
/// that is still active and that repository keeps its submodules updated, reads them again
/// for the update.
pub(super) fn job_finished(st: &mut AppState, label: &str, workdir: Option<&Path>, ok: bool, here: bool, fx: &mut Vec<Effect>) {
    let Some((w, l)) = st.submodules.after_job.take() else { return };
    if l != label || !workdir.is_some_and(|j| same_spelling(j, &w)) || !ok || !here || !auto_wanted(st) {
        return;
    }
    fx.push(Effect::Submodule(SubmoduleEffect::CheckAll { workdir: w, batch: Batch::Auto }));
}

/// After a load: a repository just opened whose submodules aren't checked out is offered to
/// initialize them, once per session, or has them initialized when it keeps them updated.
pub(super) fn opened(st: &mut AppState, kind: LoadKind, now: Instant, fx: &mut Vec<Effect>) {
    if kind != LoadKind::Open {
        return;
    }
    let Some(snap) = st.snap.as_ref() else { return };
    let targets = plan(Batch::InitAll, &snap.submodules).targets.len();
    if targets == 0 || st.submodules.offered.iter().any(|p| same_spelling(p, &snap.real_workdir)) {
        return;
    }
    let (real, workdir, name, list) = (snap.real_workdir.clone(), snap.workdir.clone(), snap.name.clone(), snap.submodules.clone());
    st.submodules.offered.push(real.clone());
    if st.settings.keeps_submodules_updated(&real) && st.runs_git_unasked() && st.job.is_none() {
        run(st, Batch::InitAll, &list, true, fx);
        return;
    }
    // Initializing can't leave anything behind, and the list was just read: no second look.
    let msg = Msg::Submodule(SubmoduleMsg::Batch { batch: Batch::InitAll, confirmed: true });
    let action = ToastAction { label: "Initialize".into(), msg: Box::new(msg), workdir: Some(workdir) };
    st.toast_action(true, offer_text(&name, targets), action, now);
}

/// The header menu's choice for the active repository, saved with the settings.
pub(super) fn set_updates(st: &mut AppState, choice: SubmoduleUpdates, fx: &mut Vec<Effect>) {
    let Some(real) = st.snap.as_ref().map(|s| s.real_workdir.clone()) else { return };
    st.settings.set_submodule_updates(&real, choice.as_override());
    fx.push(super::prefs::save_settings(st));
}

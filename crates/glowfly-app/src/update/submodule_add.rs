//! The Add submodule dialog: opening it, checking its folder on disk as the user types (the
//! view only reads the answer), and running `git submodule add` as a job.

use super::ops::{self, JobSpec, Started};
use crate::effect::{Effect, SubmoduleEffect};
use crate::state::submodules::add::{AddForm, Disk, TAB_GONE, job_label};
use crate::state::trust::InheritKind;
use crate::state::{AppState, Modal};
use glowfly_core::paths::same_spelling;
use glowfly_core::submodule_ops::add_cmd;
use std::path::PathBuf;
use std::time::Duration;

/// How long after an edit the folder is checked. Each new folder text asks again after this
/// pause; answers for text that has changed since are dropped.
pub(crate) const ADD_CHECK_DELAY: Duration = Duration::from_millis(250);

pub(super) fn dialog(st: &mut AppState) {
    let Some(snap) = st.snap.as_ref() else { return };
    st.modal = Some(Modal::AddSubmodule(AddForm::new(snap.workdir.clone())));
}

/// The URL or folder text changed: asks the runner about the folder, unless the text is
/// already refused or was asked about last.
pub(super) fn edited(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(snap) = st.snap.as_ref() else { return };
    let Some(Modal::AddSubmodule(form)) = st.modal.as_mut() else { return };
    let Ok(rel) = form.rel(snap) else { return };
    if form.asked.as_deref() == Some(rel.as_str()) {
        return;
    }
    form.asked = Some(rel.clone());
    fx.push(Effect::Submodule(SubmoduleEffect::CheckAddPath { repo: form.repo.clone(), path: rel, delay: ADD_CHECK_DELAY }));
}

/// The disk check's answer: kept only while the dialog for `repo` is open with the same folder.
pub(super) fn path_checked(st: &mut AppState, repo: PathBuf, path: String, result: Result<Vec<String>, String>) {
    let Some(snap) = st.snap.as_ref() else { return };
    match st.modal.as_mut() {
        Some(Modal::AddSubmodule(form)) if same_spelling(&form.repo, &repo) && form.rel(snap).as_deref() == Ok(path.as_str()) => {
            form.checked = Some((path, result));
        }
        _ => tracing::debug!(%path, "dropped a submodule folder check: its dialog was closed or the text changed"),
    }
}

/// The dialog's Add submodule: checks the form again and runs the job. The dialog closes only
/// once the job has started; a trust prompt holding it takes the dialog's place.
pub(super) fn submit(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(Modal::AddSubmodule(form)) = st.modal.as_ref() else { return };
    if !st.workdir().is_some_and(|w| same_spelling(&w, &form.repo)) {
        st.log(false, TAB_GONE);
        close(st);
        return;
    }
    let Some(snap) = st.snap.as_ref() else { return };
    let req = match form.request(snap) {
        Ok(r) => r,
        Err(e) => return st.log(false, e),
    };
    if !matches!(form.disk_state(&req.path), Disk::Ok(_)) {
        return;
    }
    let cmd = add_cmd(&req.url, &req.path, req.branch.as_deref());
    let (label, folder) = (job_label(&req.path), super::submodules::folder_of(&form.repo, &req.path));
    if ops::start_job(st, JobSpec::new(label.clone(), vec![cmd]), fx) == Started::Now {
        super::trust_inherit::expect(st, &label, InheritKind::Submodule, vec![folder]);
        close(st);
    }
}

fn close(st: &mut AppState) {
    if matches!(st.modal, Some(Modal::AddSubmodule(_))) {
        st.modal = None;
    }
}

//! The Edit submodule dialog: opening it prefilled with the submodule's .gitmodules URL and
//! branch, and running `git submodule set-url` / `sync` / `set-branch` as one job.

use super::ops::{self, JobSpec, Started};
use crate::effect::Effect;
use crate::state::submodules::edit::{EditForm, TAB_GONE, job_label};
use crate::state::submodules::edit_blocked;
use crate::state::{AppState, Modal};
use glowfly_core::git_version::Feature;
use glowfly_core::paths::same_spelling;

pub(super) fn dialog(st: &mut AppState, path: String) {
    let Some(sm) = super::submodules::find(st, &path) else { return };
    if let Some(why) = edit_blocked(&sm, st.git.blocked(Feature::SubmoduleEdit).as_deref()) {
        st.log(false, format!("Can't edit {path}: {why}"));
        return;
    }
    let Some(workdir) = st.workdir() else { return };
    st.modal = Some(Modal::EditSubmodule(EditForm::new(workdir, &sm)));
}

/// The dialog's Save: checks the form again and runs the job. The dialog closes once the job
/// has started; a trust prompt holding it takes the dialog's place.
pub(super) fn submit(st: &mut AppState, fx: &mut Vec<Effect>) {
    let Some(Modal::EditSubmodule(form)) = st.modal.as_ref() else { return };
    if !st.workdir().is_some_and(|w| same_spelling(&w, &form.repo)) {
        st.log(false, TAB_GONE);
        close(st);
        return;
    }
    let label = job_label(&form.path);
    let cmds = match form.commands() {
        Ok(c) => c,
        Err(e) => return st.log(false, e),
    };
    if ops::start_job(st, JobSpec::new(label, cmds), fx) == Started::Now {
        close(st);
    }
}

fn close(st: &mut AppState) {
    if matches!(st.modal, Some(Modal::EditSubmodule(_))) {
        st.modal = None;
    }
}

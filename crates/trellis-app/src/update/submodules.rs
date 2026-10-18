//! The SUBMODULES section: opening a submodule in a tab, and initializing or updating one as a
//! git job. Updating one that is checked out at another commit reads it again first, then asks,
//! naming any commits on no branch or tag that the update would leave behind. Adding one is in
//! `submodule_add.rs`, editing one in `submodule_edit.rs`, removing one in `submodule_remove.rs`,
//! and updating several at once (also after checkouts and pulls) in `submodule_batch.rs`.

use super::ops::{JobSpec, Started, start_job};
use super::{submodule_add as add, submodule_batch as batch, submodule_edit as edit, submodule_remove as remove};
use crate::effect::{Effect, SubmoduleEffect};
use crate::msg::Msg;
use crate::state::submodules::{confirm_update_body, confirm_update_button, job_label, open_blocked, update_args, update_blocked};
use crate::state::trust::InheritKind;
use crate::state::{AppState, Modal};
use crate::submodule::SubmoduleMsg;
use std::path::{Path, PathBuf};
use std::time::Instant;
use trellis_core::repo::SubmoduleInfo;

pub(super) fn handle(st: &mut AppState, msg: SubmoduleMsg, now: Instant, fx: &mut Vec<Effect>) {
    match msg {
        SubmoduleMsg::Open(path) => {
            let Some(sm) = find(st, &path) else { return };
            if let Some(why) = open_blocked(&sm) {
                st.log(false, format!("Can't open {path}: {why}"));
                return;
            }
            let Some(workdir) = st.workdir() else { return };
            super::tabs::open(st, folder_of(&workdir, &sm.path), fx);
        }
        SubmoduleMsg::Update { path, confirmed } => {
            let Some(sm) = find(st, &path) else { return };
            if let Some(why) = update_blocked(&sm) {
                st.log(false, format!("Can't update {path}: {why}"));
                return;
            }
            if sm.is_out_of_sync() && !confirmed {
                // Read it again: the commits left behind are only listed on loads, so one made
                // inside since then (on a detached HEAD, say) could be missing from the warning.
                let Some(workdir) = st.workdir() else { return };
                fx.push(Effect::Submodule(SubmoduleEffect::Check { workdir, path }));
                return;
            }
            update(st, &sm, fx);
        }
        SubmoduleMsg::Checked { path, result } => {
            let sm = match result {
                Ok(Some(sm)) => sm,
                Ok(None) => return st.log(false, format!("Submodule {path} is no longer in this repository.")),
                Err(e) => return st.log(false, format!("Could not read submodule {path}: {e}")),
            };
            // Keep the row current with what was just read.
            if let Some(row) = st.snap.as_mut().and_then(|s| s.submodules.iter_mut().find(|m| m.path == path)) {
                *row = sm.clone();
            }
            if let Some(why) = update_blocked(&sm) {
                st.log(false, format!("Can't update {path}: {why}"));
                return;
            }
            if !sm.is_out_of_sync() {
                // Its folder went away meanwhile: initializing it can't lose anything.
                update(st, &sm, fx);
                return;
            }
            st.modal = Some(Modal::danger(
                "Update submodule",
                confirm_update_body(&sm),
                confirm_update_button(&sm),
                Msg::Submodule(SubmoduleMsg::Update { path, confirmed: true }),
            ));
        }
        SubmoduleMsg::AddDialog => add::dialog(st),
        SubmoduleMsg::AddEdited => add::edited(st, fx),
        SubmoduleMsg::AddPathChecked { repo, path, result } => add::path_checked(st, repo, path, result),
        SubmoduleMsg::Add => add::submit(st, fx),
        SubmoduleMsg::RemoveDialog(path) => remove::dialog(st, path, fx),
        SubmoduleMsg::RemoveChecked { path, result } => remove::checked(st, path, result),
        SubmoduleMsg::Remove => remove::confirm(st, fx),
        SubmoduleMsg::Removed { request, report } => remove::removed(st, request, report, now, fx),
        SubmoduleMsg::Batch { batch: b, confirmed } => batch::batch(st, b, confirmed, fx),
        SubmoduleMsg::CheckedAll { batch: b, result } => batch::checked_all(st, b, result, fx),
        SubmoduleMsg::SetUpdates(choice) => batch::set_updates(st, choice, fx),
        SubmoduleMsg::EditDialog(path) => edit::dialog(st, path),
        SubmoduleMsg::Edit => edit::submit(st, fx),
    }
}

/// Initializes or updates `sm` as a job. One that wasn't checked out is trusted once checked
/// out when the repository is (trust_inherit.rs).
fn update(st: &mut AppState, sm: &SubmoduleInfo, fx: &mut Vec<Effect>) {
    let label = job_label(sm);
    if start_job(st, JobSpec::new(label.clone(), vec![update_args(&sm.path)]), fx) == Started::Now
        && !sm.is_initialized()
        && let Some(workdir) = st.workdir()
    {
        super::trust_inherit::expect(st, &label, InheritKind::Submodule, vec![folder_of(&workdir, &sm.path)]);
    }
}

/// The folder of the submodule at `path` (slashes) in the repository at `workdir`, one
/// component at a time so it uses the platform's separators.
pub(super) fn folder_of(workdir: &Path, path: &str) -> PathBuf {
    path.split('/').fold(workdir.to_path_buf(), |p, c| p.join(c))
}

/// The submodule at `path` in the active tab's snapshot; logs when it is gone.
pub(super) fn find(st: &mut AppState, path: &str) -> Option<SubmoduleInfo> {
    let found = st.snap.as_ref().and_then(|s| s.submodules.iter().find(|m| m.path == path).cloned());
    if found.is_none() {
        st.log(false, format!("Submodule {path} is no longer in this repository."));
    }
    found
}

//! The SUBMODULES section: opening a submodule in a tab, and initializing or updating one as a
//! git job, with a confirmation first when it is checked out at another commit.

use super::ops::{JobSpec, start_job};
use crate::effect::Effect;
use crate::msg::Msg;
use crate::state::submodules::{confirm_update_body, job_label, open_blocked, update_args, update_blocked};
use crate::state::{AppState, Modal};
use crate::submodule::SubmoduleMsg;
use trellis_core::repo::SubmoduleInfo;

pub(super) fn handle(st: &mut AppState, msg: SubmoduleMsg, fx: &mut Vec<Effect>) {
    match msg {
        SubmoduleMsg::Open(path) => {
            let Some(sm) = find(st, &path) else { return };
            if let Some(why) = open_blocked(&sm) {
                st.log(false, format!("Can't open {path}: {why}"));
                return;
            }
            let Some(workdir) = st.workdir() else { return };
            // One component at a time, so the folder uses the platform's separators.
            let folder = sm.path.split('/').fold(workdir, |p, c| p.join(c));
            super::tabs::open(st, folder, fx);
        }
        SubmoduleMsg::Update { path, confirmed } => {
            let Some(sm) = find(st, &path) else { return };
            if let Some(why) = update_blocked(&sm) {
                st.log(false, format!("Can't update {path}: {why}"));
                return;
            }
            if sm.is_out_of_sync() && !confirmed {
                st.modal = Some(Modal::Confirm {
                    title: "Update submodule".into(),
                    body: confirm_update_body(&sm),
                    button: "Update".into(),
                    then: Box::new(Msg::Submodule(SubmoduleMsg::Update { path, confirmed: true })),
                });
                return;
            }
            start_job(st, JobSpec::new(job_label(&sm), vec![update_args(&sm.path)]), fx);
        }
    }
}

/// The submodule at `path` in the active tab's snapshot; logs when it is gone.
fn find(st: &mut AppState, path: &str) -> Option<SubmoduleInfo> {
    let found = st.snap.as_ref().and_then(|s| s.submodules.iter().find(|m| m.path == path).cloned());
    if found.is_none() {
        st.log(false, format!("Submodule {path} is no longer in this repository."));
    }
    found
}

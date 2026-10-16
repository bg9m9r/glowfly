//! The REMOTE section: Add and Edit remote (the dialog's form checked again against the current
//! snapshot), Fetch one remote, and Remove after a confirmation. Every change is a plain git
//! job through `ops::start_job`. View on service and the fork dialogs are in `remote_forks`.

use super::ops::{self, JobSpec};
use super::remote_forks;
use crate::effect::Effect;
use crate::msg::Msg;
use crate::remote::RemoteMsg;
use crate::state::remotes::{RemoteForm, find, gone_text, remove_body, request};
use crate::state::{AppState, Modal};
use trellis_core::remote_ops::{fetch_cmd, remove_cmd};
use trellis_core::repo::RemoteInfo;

pub(super) fn handle(st: &mut AppState, msg: RemoteMsg, fx: &mut Vec<Effect>) {
    match msg {
        RemoteMsg::AddDialog => {
            let Some(snap) = st.snap.as_ref() else { return };
            st.modal = Some(Modal::Remote(RemoteForm::add(snap)));
        }
        RemoteMsg::EditDialog(name) => {
            let Some(info) = configured(st, &name) else { return };
            st.modal = Some(Modal::Remote(RemoteForm::edit(&info)));
        }
        RemoteMsg::Submit => submit(st, fx),
        RemoteMsg::Fetch(name) => {
            let Some(info) = configured(st, &name) else { return };
            if info.fetch_url().is_empty() {
                return st.log(false, format!("Can't fetch {name}: it has no fetch URL."));
            }
            ops::start_job(st, JobSpec::new(format!("Fetch {name}"), vec![fetch_cmd(&name)]), fx);
        }
        RemoteMsg::RemoveDialog(name) => {
            if configured(st, &name).is_none() {
                return;
            }
            let Some(body) = st.snap.as_ref().map(|s| remove_body(s, &name)) else { return };
            st.modal = Some(Modal::Confirm {
                title: format!("Remove remote {name}"),
                body,
                button: "Remove remote".into(),
                then: Box::new(Msg::Remote(RemoteMsg::Remove(name))),
            });
        }
        RemoteMsg::Remove(name) => {
            if configured(st, &name).is_none() {
                return;
            }
            ops::start_job(st, JobSpec::new(format!("Remove remote {name}"), vec![remove_cmd(&name)]), fx);
        }
        RemoteMsg::View(name) => remote_forks::view(st, &name, fx),
        RemoteMsg::ForkDialog(name) => remote_forks::open_picker(st, &name, fx),
        RemoteMsg::PickFork(i) => remote_forks::pick(st, i),
        RemoteMsg::AddFork => remote_forks::add_fork(st, fx),
        RemoteMsg::ForkOnService(name) => remote_forks::open_create(st, &name, fx),
        RemoteMsg::AddMyFork => remote_forks::add_my_fork(st, fx),
        RemoteMsg::CreateFork => remote_forks::create(st, fx),
        RemoteMsg::RetryForks => remote_forks::retry(st, fx),
        RemoteMsg::ForksLoaded { workdir, target, result } => remote_forks::forks_loaded(st, &workdir, &target, *result),
        RemoteMsg::MyForkFound { workdir, target, result } => remote_forks::my_fork_found(st, &workdir, &target, *result),
        RemoteMsg::ForkCreated { workdir, target, name, result } => remote_forks::fork_created(st, workdir, &target, name, *result, fx),
    }
}

/// The configured remote `name` in the current snapshot, or None after logging that it is gone.
pub(super) fn configured(st: &mut AppState, name: &str) -> Option<RemoteInfo> {
    let found = find(st.snap.as_ref()?, name).cloned();
    if found.is_none() {
        st.log(false, gone_text(name));
    }
    found
}

/// The dialog's Add remote / Save: checked again; on a problem the dialog stays open.
fn submit(st: &mut AppState, fx: &mut Vec<Effect>) {
    let (Some(snap), Some(Modal::Remote(form))) = (st.snap.as_ref(), st.modal.as_ref()) else { return };
    match request(snap, form) {
        Err(e) => st.log(false, e),
        Ok(None) => st.modal = None,
        Ok(Some((label, cmds))) => {
            let before = fx.len();
            ops::start_job(st, JobSpec::new(label, cmds), fx);
            // Busy: start_job logged why and the dialog stays open.
            if fx.len() > before {
                st.modal = None;
            }
        }
    }
}

//! Push to your fork…: the create dialog's way to a pull request from the user's own fork. It
//! opens the Fork dialog (update/remote_forks.rs) for the base repository with `then_pr` set;
//! adding the fork then also pushes the branch to it in the same job, and the create dialog
//! opens from the fork once the push has landed (update/push_pr.rs).

use super::ops::{JobSpec, start_job};
use crate::effect::Effect;
use crate::state::forks::{ForkStep, ForkThenPr, add_job, added_as};
use crate::state::pr_repos;
use crate::state::push_pr::PrStart;
use crate::state::remotes::{find, gone_text};
use crate::state::{AppState, Modal};
use glowfly_core::branch_ops::push_cmd;
use glowfly_core::remote_ops::add_cmds;
use glowfly_core::repo::Snapshot;

/// Opens the Fork dialog for the base repository, to push local `branch` to the fork; when
/// the dialog can't open, `open_create` logged why.
pub(super) fn open(st: &mut AppState, branch: String, target: Option<String>, fx: &mut Vec<Effect>) {
    let (Some(snap), Some(fs)) = (&st.snap, &st.forge) else { return };
    if !snap.locals.iter().any(|b| b.name == branch) {
        st.log(false, format!("Branch {branch} no longer exists."));
        return;
    }
    let base = pr_repos::base_remote(fs).to_owned();
    super::remote_forks::open_create(st, &base, fx);
    if let Some(Modal::CreateFork(f)) = st.modal.as_mut() {
        f.then_pr = Some(ForkThenPr { branch, target });
    }
}

/// The job that adds the fork as remote `name` at `url` (fetching it when `fetch`) and pushes
/// the branch there with -u, then opens the create dialog from it; None when the branch is gone.
pub(super) fn add_and_push_job(snap: &Snapshot, name: &str, url: &str, fetch: bool, then: &ForkThenPr) -> Option<JobSpec> {
    let b = snap.locals.iter().find(|b| b.name == then.branch)?;
    let mut cmds = add_cmds(name, url, None, fetch);
    // A new remote is never the base repository, so the branch's upstream moves to the fork.
    cmds.push(push_cmd(&b.name, name, &b.name, true));
    let then_pr = PrStart { source: b.name.clone(), target: then.target.clone(), remote: Some(name.to_owned()) };
    Some(JobSpec { then_pr: Some(then_pr), ..JobSpec::new(format!("Add remote {name} and push {}", b.name), cmds) })
}

/// Pushes the branch to the fork that is already remote `remote`.
pub(super) fn push_existing(st: &mut AppState, then: ForkThenPr, remote: String, fx: &mut Vec<Effect>) {
    super::push_pr::start(st, &then.branch, then.target, Some(remote), fx);
}

/// The Fork dialog's primary on a fork the user already has, in then-push mode: push to it
/// when it already is a remote, else add it and push in one job. The dialog closes once that
/// starts.
pub(super) fn add_mine(st: &mut AppState, then: ForkThenPr, fx: &mut Vec<Effect>) {
    enum Next {
        Push(String),
        Run(Box<JobSpec>),
        Refuse(String),
    }
    let next = {
        let (Some(snap), Some(Modal::CreateFork(f))) = (st.snap.as_ref(), st.modal.as_ref()) else { return };
        let ForkStep::Mine { fork, .. } = &f.step else { return };
        let connections = &st.settings.connections;
        match (added_as(snap, fork, connections), find(snap, &f.remote)) {
            (Some(r), _) => Next::Push(r),
            (None, None) => Next::Refuse(gone_text(&f.remote)),
            (None, Some(src)) => match add_job(snap, src.fetch_url(), fork, &f.name, f.fetch_after, connections) {
                Err(e) => Next::Refuse(e),
                Ok(_) => match add_and_push_job(snap, f.name.trim(), fork.url_like(src.fetch_url()), f.fetch_after, &then) {
                    Some(spec) => Next::Run(Box::new(spec)),
                    None => Next::Refuse(format!("Branch {} no longer exists.", then.branch)),
                },
            },
        }
    };
    match next {
        Next::Refuse(why) => st.log(false, why),
        Next::Push(remote) => {
            st.modal = None;
            push_existing(st, then, remote, fx);
        }
        Next::Run(spec) => {
            let before = fx.len();
            start_job(st, *spec, fx);
            if fx.len() > before {
                st.modal = None;
            }
        }
    }
}

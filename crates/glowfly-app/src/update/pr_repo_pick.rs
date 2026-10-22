//! The create dialog's From / Into repository choice (pull requests across forks): picking
//! From renames the source to its name on that remote, picking Into reloads the target
//! branches from that remote; both keep the chosen target when it is still listed.

use crate::state::pr_repos::{self, RepoSide};
use crate::state::push_pr;
use crate::state::{AppState, Modal};

/// Applies a From / Into pick; a remote that isn't a PR repository, or no open create dialog,
/// changes nothing.
pub(super) fn pick(st: &mut AppState, side: RepoSide, remote: &str) {
    let (Some(snap), Some(fs)) = (st.snap.as_ref(), st.forge.as_ref()) else { return };
    if !pr_repos::is_pr_remote(fs, remote) {
        return;
    }
    let Some(Modal::CreatePr { source, target, targets, repos, .. }) = st.modal.as_mut() else { return };
    match side {
        RepoSide::From => {
            repos.from = remote.to_owned();
            *source = push_pr::pr_source_on(snap, remote, &repos.branch);
        }
        RepoSide::Into => repos.into = remote.to_owned(),
    }
    // Whether the source is left out of the targets depends on From and Into, so both reload.
    let (from, into) = (repos.remote_from(fs).to_owned(), repos.remote_into(fs).to_owned());
    *targets = pr_repos::target_branches(snap, fs, &into, &from, source);
    if !targets.contains(target) {
        *target = pr_repos::default_target(snap, fs, &into, targets, source, &from, None);
    }
}

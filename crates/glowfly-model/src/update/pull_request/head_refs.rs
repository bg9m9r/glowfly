//! The refs that keep a pull request's head while a panel shows it (`refs/glowfly/pr/<id>`,
//! `glowfly_core::pr_head_refs`): a ref is held when the head fetch starts or answers, or when the
//! files are compared from a head that was already here (source.rs); `settle` runs after every
//! update and deletes the refs no panel copy shows any more, whichever way the panel went; and
//! `sweep` deletes the leftovers of an earlier run when a repository opens.

use crate::effect::Effect;
use crate::pull_request::{HeadRefJob, PrEffect};
use crate::state::AppState;
use crate::state::pull_request::head_refs::{HeldRef, take_unused};
use crate::state::tabs::is_repo_at;
use glowfly_core::repo::Snapshot;
use std::path::Path;

/// Holds pull request `id`'s ref in the repository at `workdir`.
pub(super) fn hold(st: &mut AppState, workdir: &Path, id: u64) {
    let common = match &st.snap {
        Some(snap) if is_repo_at(snap, workdir) => Some(snap.common.clone()),
        _ => st
            .tabs
            .iter()
            .find(|t| t.is(workdir))
            .and_then(|t| t.parked.as_ref().and_then(|p| p.snap.as_ref()).map(|s| s.common.clone()).or_else(|| t.group.clone())),
    };
    st.pull_requests.head_refs.hold(HeldRef { workdir: workdir.to_owned(), common, id });
}

/// Deletes the held refs no panel copy shows: after every update, so every way the panel goes
/// (Close, another pull request, a view replacing it, the tab opening another repository or
/// closing) is covered at once.
pub(in crate::update) fn settle(st: &mut AppState, fx: &mut Vec<Effect>) {
    if st.pull_requests.head_refs.held.is_empty() {
        return;
    }
    for (workdir, ids) in take_unused(st) {
        fx.push(Effect::PullRequest(PrEffect::HeadRef(HeadRefJob::Drop { workdir, ids })));
    }
}

/// A repository opened: every other ref under the prefix is a leftover of a crash, a quit or
/// another window. The ones this run still holds for it stay.
pub(in crate::update) fn sweep(st: &AppState, snap: &Snapshot, fx: &mut Vec<Effect>) {
    let keep = st.pull_requests.head_refs.held_ids(&snap.common, &snap.workdir);
    fx.push(Effect::PullRequest(PrEffect::HeadRef(HeadRefJob::Sweep { workdir: snap.workdir.clone(), keep })));
}

/// A head fetch for a tab that isn't the active one ended: its ref is held so that `settle`
/// deletes it once no panel shows it.
pub(in crate::update) fn fetched_elsewhere(st: &mut AppState, workdir: &Path, id: u64) {
    hold(st, workdir, id);
}

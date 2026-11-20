//! Which `refs/glowfly/pr/<id>` refs this run holds, one per repository folder and pull request,
//! and whether a panel still shows the pull request (`glowfly_core::pr_head_refs`). A ref is
//! held from the moment the head is fetched or kept, and given up once no panel copy in that
//! repository shows it: the active tab's, or any parked tab's, where worktree tabs share one
//! namespace (their common git dir). Pure: update/pull_request/head_refs.rs runs the effects.

use crate::state::AppState;
use crate::state::tabs::is_repo_at;
use glowfly_core::paths::same_spelling;
use glowfly_core::repo::CommonDir;
use std::path::{Path, PathBuf};

/// A ref this run wrote or asked for: pull request `id` of the repository at `workdir`.
#[derive(Clone, Debug, PartialEq)]
pub struct HeldRef {
    pub workdir: PathBuf,
    /// The repository's common git dir, when known: what ties worktree tabs together.
    pub common: Option<CommonDir>,
    pub id: u64,
}

/// The refs held.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeadRefs {
    pub held: Vec<HeldRef>,
}

impl HeadRefs {
    /// Holds `r`, unless that pull request is held for the same folder already.
    pub fn hold(&mut self, r: HeldRef) {
        if !self.held.iter().any(|h| h.id == r.id && same_spelling(&h.workdir, &r.workdir)) {
            self.held.push(r);
        }
    }

    /// The pull requests held in the repository `common` (whose folder here is `workdir`), sorted.
    pub fn held_ids(&self, common: &CommonDir, workdir: &Path) -> Vec<u64> {
        let mut ids: Vec<u64> =
            self.held.iter().filter(|h| same_repo(h, Some(common), same_spelling(&h.workdir, workdir))).map(|h| h.id).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}

/// Whether a panel copy of the repository with `common` (`here`: its folder is the held ref's)
/// is in the repository `held` is in.
fn same_repo(held: &HeldRef, common: Option<&CommonDir>, here: bool) -> bool {
    here || matches!((&held.common, common), (Some(a), Some(b)) if a.same(b))
}

/// Whether any panel copy in the repository of `r` shows its pull request: the active tab's, or
/// a parked tab's. A kept draft isn't an open panel.
pub fn shows(st: &AppState, r: &HeldRef) -> bool {
    let open = |s: &crate::state::session::RepoSession| s.pull_request.view.as_ref().is_some_and(|v| v.pr.id == r.id);
    let active = st.snap.as_ref().is_some_and(|snap| open(&st.session) && same_repo(r, Some(&snap.common), is_repo_at(snap, &r.workdir)));
    active
        || st.tabs.iter().any(|t| {
            let Some(p) = t.parked.as_deref().filter(|p| open(&p.session)) else { return false };
            let common = p.snap.as_ref().map(|s| &s.common).or(t.group.as_ref());
            same_repo(r, common, t.is(&r.workdir))
        })
}

/// Removes the refs no panel copy shows any more and returns their ids by folder, in the order
/// they were held.
pub fn take_unused(st: &mut AppState) -> Vec<(PathBuf, Vec<u64>)> {
    let held = std::mem::take(&mut st.pull_requests.head_refs.held);
    let (keep, gone): (Vec<HeldRef>, Vec<HeldRef>) = held.into_iter().partition(|r| shows(st, r));
    st.pull_requests.head_refs.held = keep;
    let mut by_folder: Vec<(PathBuf, Vec<u64>)> = Vec::new();
    for r in gone {
        match by_folder.iter_mut().find(|(w, _)| same_spelling(w, &r.workdir)) {
            Some((_, ids)) => ids.push(r.id),
            None => by_folder.push((r.workdir, vec![r.id])),
        }
    }
    by_folder
}

//! Restacking stacked branches: the plan a request gives on the loaded snapshot, why it can't
//! run (the same text for the branch menu's disabled items and `update`'s refusals), how the
//! undo history names it, and restacks stopped on a conflict that wait for the banner.

use crate::effect::RestackJob;
use std::path::PathBuf;
use trellis_core::i18n::Lang;
use trellis_core::repo::Snapshot;
use trellis_core::stack::{RestackPlan, RestackScope, RestackStep, plan};

/// A restack stopped on a conflict in the repository at `workdir`, with the steps left once the
/// banner's Continue or Skip finishes the stopped one.
#[derive(Debug, Clone)]
pub struct PendingRestack {
    pub workdir: PathBuf,
    pub job: RestackJob,
}

/// The steps `scope` asks for on `snap`'s stacks.
pub fn plan_for(snap: &Snapshot, scope: &RestackScope, autostash: bool) -> RestackPlan {
    plan(&snap.stacks, scope, snap.head_branch.as_deref(), autostash)
}

/// The branches a restack dropped after a stop didn't get to: the top of the step it stopped
/// in, when a layer of that step is still behind on `snap` (its rebase was aborted rather than
/// finished outside Trellis), then the tops of the steps left.
pub fn not_restacked(snap: &Snapshot, job: &RestackJob) -> Vec<String> {
    let unfinished = |s: &&RestackStep| {
        let one = RestackPlan { steps: vec![(*s).clone()], ..RestackPlan::default() };
        one.moved(&snap.stacks).iter().any(|b| snap.stacks.below(b).is_some_and(|l| l.behind))
    };
    job.stopped.iter().filter(unfinished).chain(&job.plan.steps).map(|s| s.top.clone()).collect()
}

/// The branch the layers move onto: `b` for the branches above it, else the branch `x` is
/// stacked on (`x` itself when it isn't stacked).
pub fn lower(snap: &Snapshot, scope: &RestackScope) -> String {
    match scope {
        RestackScope::Above(b) => b.clone(),
        RestackScope::Onto(x) => snap.stacks.below(x).map_or_else(|| x.clone(), |l| l.on.clone()),
    }
}

/// How the undo history names the restack: "restack of the branches above search" or
/// "restack of login-page onto search".
pub fn detail(snap: &Snapshot, scope: &RestackScope) -> String {
    match scope {
        RestackScope::Above(b) => format!("restack of the branches above {b}"),
        RestackScope::Onto(x) => format!("restack of {x} onto {}", lower(snap, scope)),
    }
}

/// Why `scope` can't be restacked now: an operation in progress, nothing behind, or a branch
/// it would move is checked out in another worktree (git would leave it where it is).
pub fn blocker(t: Lang, snap: &Snapshot, scope: &RestackScope) -> Option<String> {
    if let Some(why) = super::rebase_onto::in_progress(t, snap.state) {
        return Some(why.to_owned());
    }
    let plan = plan_for(snap, scope, false);
    if plan.is_empty() {
        return Some(t.trf("stack.blocked.up_to_date", &[("branch", &lower(snap, scope))]));
    }
    let busy = plan.moved(&snap.stacks).into_iter().find(|n| super::worktree_branches::elsewhere(snap, n).is_some())?;
    Some(t.trf("stack.blocked.worktree", &[("branch", &busy)]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Oid, RepositoryState};
    use trellis_core::stack::{StackLink, Stacks};

    fn stacked(name: &str, behind: bool) -> Snapshot {
        let mut snap = crate::state::op_fixture::snapshot(name);
        let base = snap.head_oid.unwrap_or(Oid::zero());
        snap.stacks = Stacks { links: vec![StackLink { branch: "login-page".into(), on: "search".into(), base, behind }] };
        snap
    }

    #[test]
    fn blockers_are_localised() {
        let above = RestackScope::Above("search".into());
        let mut snap = stacked("stack-blockers", false);
        let en = blocker(Lang::En, &snap, &above).unwrap();
        assert_eq!(en, "The branches above search are already on its tip.");
        assert_ne!(blocker(Lang::De, &snap, &above).as_deref(), Some(en.as_str()));
        assert!(blocker(Lang::De, &snap, &above).unwrap().contains("search"));

        snap.stacks.links[0].behind = true;
        assert_eq!(blocker(Lang::En, &snap, &above), None);
        assert_eq!(blocker(Lang::En, &snap, &RestackScope::Onto("login-page".into())), None);

        let mut other = snap.worktrees[0].clone();
        (other.current, other.branch, other.path) = (false, Some("login-page".into()), snap.workdir.join("other"));
        snap.worktrees.push(other);
        let en = blocker(Lang::En, &snap, &above).unwrap();
        assert_eq!(en, "login-page is checked out in another worktree; restack it from there.");
        assert_ne!(blocker(Lang::De, &snap, &above), Some(en));
        snap.worktrees.truncate(1);

        snap.state = RepositoryState::Merge;
        let busy = blocker(Lang::En, &snap, &above).unwrap();
        assert_eq!(Some(busy.as_str()), super::super::rebase_onto::in_progress(Lang::En, RepositoryState::Merge));
        assert_ne!(blocker(Lang::De, &snap, &above), Some(busy));
    }

    #[test]
    fn details_name_the_scope() {
        let snap = stacked("stack-details", true);
        assert_eq!(detail(&snap, &RestackScope::Above("search".into())), "restack of the branches above search");
        assert_eq!(detail(&snap, &RestackScope::Onto("login-page".into())), "restack of login-page onto search");
    }
}

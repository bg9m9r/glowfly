//! Continuing a Finish that stopped at a merge conflict: the steps still to run once the merge is
//! committed, whether a snapshot shows that commit (or shows the remaining steps can no longer
//! apply), and the checks before the remaining steps run. Pure: reads only the snapshot.

use super::finish::{check_tag, checkout_step};
use super::{Branches, FinishPlan, FinishProblem, GitFlowSettings, StepAction, WorkState, flow_kind};
use crate::rebase::operation_noun;
use crate::repo::Snapshot;
use git2::{Oid, RepositoryState};
use std::collections::{HashSet, VecDeque};

fn local_oid(snap: &Snapshot, name: &str) -> Option<Oid> {
    snap.locals.iter().find(|b| b.name == name).and_then(|b| b.oid)
}

/// The tips when a Finish (or Continue) job started: the flow branch and every branch it merges
/// into.
#[derive(Clone, Debug, PartialEq)]
pub struct StartTips {
    pub branch: Oid,
    pub bases: Vec<(String, Oid)>,
}

impl StartTips {
    /// None when the flow branch is missing or unborn.
    pub fn of(snap: &Snapshot, plan: &FinishPlan) -> Option<Self> {
        let branch = local_oid(snap, &plan.branch)?;
        let bases = plan
            .steps
            .iter()
            .filter_map(|s| match &s.action {
                StepAction::Merge { into } => local_oid(snap, into).map(|o| (into.clone(), o)),
                _ => None,
            })
            .collect();
        Some(StartTips { branch, bases })
    }
}

/// A Finish stopped by a conflict in its merge into `into`.
#[derive(Clone, Debug, PartialEq)]
pub struct FinishResume {
    /// The plan with only the steps still to run.
    pub plan: FinishPlan,
    /// The conflicted merge's target.
    pub into: String,
    /// The flow branch's tip when the job started (the merge's MERGE_HEAD).
    pub tip: Oid,
    /// The target's tip before the merge.
    pub base: Oid,
}

impl FinishPlan {
    /// What is left when step `failed` stopped with a conflict: Some only when that step is a
    /// merge with steps after it and `tips` has its target.
    pub fn rest_after_conflict(&self, failed: usize, tips: &StartTips) -> Option<FinishResume> {
        let StepAction::Merge { into } = &self.steps.get(failed)?.action else { return None };
        if failed + 1 >= self.steps.len() {
            return None;
        }
        let base = tips.bases.iter().find(|(n, _)| n == into)?.1;
        let plan = FinishPlan { steps: self.steps[failed + 1..].to_vec(), ..self.clone() };
        Some(FinishResume { plan, into: into.clone(), tip: tips.branch, base })
    }
}

impl FinishResume {
    /// The conflicted merge is still in progress.
    pub fn merging(&self, snap: &Snapshot) -> bool {
        snap.state == RepositoryState::Merge && snap.head_branch.as_deref() == Some(self.into.as_str())
    }
}

/// Whether the remaining steps can run.
#[derive(Clone, Debug, PartialEq)]
pub enum ResumeState {
    /// The merge isn't committed yet (or the commit can't be seen in the loaded history).
    Waiting,
    /// The merge is committed.
    Ready,
    /// The remaining steps no longer apply.
    Gone(ResumeGone),
}

/// Why the remaining steps no longer apply.
#[derive(Clone, Debug, PartialEq)]
pub enum ResumeGone {
    NotSetUp,
    /// The production or development branch or the prefixes changed, so the target or the kind
    /// no longer matches.
    SettingsChanged,
    /// The flow branch was deleted.
    BranchGone(String),
    /// The flow branch got new commits.
    BranchMoved(String),
    /// The merge's target was deleted.
    BaseGone(String),
    /// The merge into this branch was aborted.
    Aborted(String),
}

impl ResumeGone {
    /// "feature/login no longer exists" (no full stop).
    pub fn message(&self) -> String {
        match self {
            ResumeGone::NotSetUp => "Git Flow is no longer set up".into(),
            ResumeGone::SettingsChanged => "the Git Flow settings changed".into(),
            ResumeGone::BranchGone(b) | ResumeGone::BaseGone(b) => format!("{b} no longer exists"),
            ResumeGone::BranchMoved(b) => format!("{b} has new commits since Finish stopped"),
            ResumeGone::Aborted(into) => format!("the merge into {into} was aborted"),
        }
    }
}

/// Where `r` stands in `snap`. `seen_merge`: a snapshot with the merge in progress was seen,
/// so a clean state with the target back at `base` means the merge was aborted (a stale
/// snapshot from before the merge doesn't).
pub fn resume_state(r: &FinishResume, seen_merge: bool, snap: &Snapshot) -> ResumeState {
    let gone = ResumeState::Gone;
    let Some(s) = snap.gitflow.settings() else { return gone(ResumeGone::NotSetUp) };
    if flow_kind(&s, &r.plan.branch) != Some(r.plan.kind) || (r.into != s.production && r.into != s.development) {
        return gone(ResumeGone::SettingsChanged);
    }
    match local_oid(snap, &r.plan.branch) {
        None => return gone(ResumeGone::BranchGone(r.plan.branch.clone())),
        Some(o) if o != r.tip => return gone(ResumeGone::BranchMoved(r.plan.branch.clone())),
        Some(_) => {}
    }
    let Some(into_tip) = local_oid(snap, &r.into) else { return gone(ResumeGone::BaseGone(r.into.clone())) };
    if snap.state != RepositoryState::Clean {
        return ResumeState::Waiting;
    }
    if reaches(snap, into_tip, r.tip) {
        ResumeState::Ready
    } else if seen_merge && into_tip == r.base {
        gone(ResumeGone::Aborted(r.into.clone()))
    } else {
        ResumeState::Waiting
    }
}

/// Whether `target` is `from` or one of its ancestors in the loaded history (a commit that
/// isn't loaded ends its path).
fn reaches(snap: &Snapshot, from: Oid, target: Oid) -> bool {
    let mut seen = HashSet::from([from]);
    let mut queue = VecDeque::from([from]);
    while let Some(oid) = queue.pop_front() {
        if oid == target {
            return true;
        }
        let Some(c) = snap.row_of.get(&oid).and_then(|&row| snap.commits.get(row)) else { continue };
        for p in &c.parents {
            if seen.insert(*p) {
                queue.push_back(*p);
            }
        }
    }
    false
}

/// The steps to run: `r`'s, after a checkout of the merge's target when a step needs HEAD there
/// before any step checks out a branch (the delete left after the merge into development: HEAD
/// may have moved since the merge was committed).
fn resume_plan(r: &FinishResume) -> FinishPlan {
    let mut plan = r.plan.clone();
    let mut before_checkout = plan.steps.iter().take_while(|s| !matches!(s.action, StepAction::Checkout(_)));
    if before_checkout.any(|s| !matches!(s.action, StepAction::Tag(_))) {
        plan.steps.insert(0, checkout_step(&r.into));
    }
    plan
}

/// The checks before the remaining steps run, in this order: still a flow branch, it exists,
/// the branches the steps use exist, no operation in progress, no tracked changes, nothing the
/// steps check out or delete is checked out elsewhere, and a tag still to make is free. Returns
/// the steps to run (see `resume_plan`).
pub fn check_resume(r: &FinishResume, s: &GitFlowSettings, b: &Branches, w: &WorkState) -> Result<FinishPlan, FinishProblem> {
    use FinishProblem as P;
    let plan = &resume_plan(r);
    if flow_kind(s, &plan.branch) != Some(plan.kind) {
        return Err(P::NotFlowBranch(plan.branch.clone()));
    }
    if !b.local(&plan.branch) {
        return Err(P::NoBranch(plan.branch.clone()));
    }
    let mut used: Vec<&str> = vec![&r.into];
    for step in &plan.steps {
        if let StepAction::Checkout(n) | StepAction::Merge { into: n } = &step.action
            && !used.contains(&n.as_str())
        {
            used.push(n);
        }
    }
    let missing: Vec<String> = used.iter().filter(|n| !b.local(n)).map(|n| n.to_string()).collect();
    if !missing.is_empty() {
        return Err(P::MissingBases { kind: plan.kind, missing });
    }
    if let Some(op) = operation_noun(w.state) {
        return Err(P::InProgress(op));
    }
    if w.tracked_changes {
        return Err(P::TrackedChanges(plan.branch.clone()));
    }
    let mut touched = plan.steps.iter().filter_map(|s| match &s.action {
        StepAction::Checkout(n) => Some(n.as_str()),
        StepAction::Delete => Some(plan.branch.as_str()),
        _ => None,
    });
    if let Some(n) = touched.find(|n| w.elsewhere.contains(n)) {
        return Err(P::CheckedOutElsewhere(n.to_owned()));
    }
    for step in &plan.steps {
        if let StepAction::Tag(t) = &step.action {
            check_tag(t, &b.tags)?;
        }
    }
    Ok(plan.clone())
}

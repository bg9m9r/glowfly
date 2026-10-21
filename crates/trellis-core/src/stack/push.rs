//! Pushing every layer of a stack through gitcli, so hooks, credentials and the user's config
//! apply. Each layer is classified first against the remote tip the caller last saw: new and
//! fast-forward layers are pushed plainly, a layer rewritten locally (that remote tip is a
//! current or former value of the branch, from its reflog) is pushed with a lease on that tip,
//! and a layer behind or diverged from the remote is never pushed. One `git push --porcelain`
//! runs per (remote, `-u`) group, and its per-ref lines give each layer's outcome.

use super::porcelain;
use crate::error::Result;
use crate::gitcli;
use git2::{BranchType, Oid, Repository};
use std::path::Path;

/// The most reflog entries read when looking for the remote tip among a branch's former
/// values; a remote tip older than that counts as diverged (refused, never forced).
pub const MAX_LEASE_REFLOG: usize = 200;

/// Local `branch` to `dest` on `remote`; `set_upstream` adds -u; `seen` is the remote branch's
/// tip the caller last saw (None: absent).
#[derive(Clone, Debug, PartialEq)]
pub struct PushTarget {
    pub branch: String,
    pub remote: String,
    pub dest: String,
    pub set_upstream: bool,
    pub seen: Option<Oid>,
}

/// How a layer relates to its remote branch before the push.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PushPlan {
    /// No remote branch yet.
    New,
    UpToDate,
    FastForward,
    /// The remote tip was once this branch's: replaced with a lease on it.
    Rewritten,
    /// The remote has commits the branch lacks; the branch has none of its own.
    Behind,
    /// The remote has commits that were never on the branch.
    Diverged,
    /// The local branch no longer exists.
    Gone,
}

impl PushPlan {
    /// Whether the layer goes into a push command.
    pub fn pushes(self) -> bool {
        matches!(self, PushPlan::New | PushPlan::FastForward | PushPlan::Rewritten)
    }
}

/// What happened to one layer.
#[derive(Clone, Debug, PartialEq)]
pub enum LayerOutcome {
    /// Pushed as planned; `upstream` when -u set its tracking.
    Pushed {
        plan: PushPlan,
        upstream: bool,
    },
    UpToDate,
    /// Not pushed: Behind, Diverged or Gone.
    NotPushed(PushPlan),
    /// The lease failed: the remote moved since the caller saw it.
    Stale,
    /// The remote (or git) refused it, with git's reason.
    Rejected(String),
    /// No answer for it: the push failed before its refs (sign-in, network, a hook).
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayerPushed {
    pub target: PushTarget,
    pub outcome: LayerOutcome,
}

/// Every layer's outcome, in the order given, and the commands with git's output.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StackPush {
    pub layers: Vec<LayerPushed>,
    pub output: String,
}

impl StackPush {
    /// Every layer ended pushed or up to date.
    pub fn ok(&self) -> bool {
        self.landed() == self.layers.len()
    }

    /// How many layers ended pushed or up to date.
    pub fn landed(&self) -> usize {
        self.layers.iter().filter(|l| matches!(l.outcome, LayerOutcome::Pushed { .. } | LayerOutcome::UpToDate)).count()
    }
}

/// How `t`'s local branch relates to the remote tip `t.seen`.
pub fn classify(repo: &Repository, t: &PushTarget) -> PushPlan {
    let Some(local) = repo.find_branch(&t.branch, BranchType::Local).ok().and_then(|b| b.get().target()) else {
        return PushPlan::Gone;
    };
    let Some(seen) = t.seen else { return PushPlan::New };
    if seen == local {
        return PushPlan::UpToDate;
    }
    if repo.find_commit(seen).is_err() {
        return PushPlan::Diverged;
    }
    let descends = |a: Oid, b: Oid| repo.graph_descendant_of(a, b).unwrap_or(false);
    if descends(local, seen) {
        PushPlan::FastForward
    } else if descends(seen, local) {
        PushPlan::Behind
    } else if rewritten(repo, &t.branch, seen) {
        PushPlan::Rewritten
    } else {
        PushPlan::Diverged
    }
}

/// Whether `seen` is a current or former value of `branch` (or an ancestor of one), from the
/// newest [`MAX_LEASE_REFLOG`] reflog entries: the branch was rewritten here.
fn rewritten(repo: &Repository, branch: &str, seen: Oid) -> bool {
    let Ok(log) = repo.reflog(&format!("refs/heads/{branch}")) else { return false };
    log.iter().take(MAX_LEASE_REFLOG).any(|e| {
        [e.id_new(), e.id_old()]
            .into_iter()
            .filter(|id| !id.is_zero())
            .any(|id| id == seen || repo.graph_descendant_of(id, seen).unwrap_or(false))
    })
}

/// Pushes `targets` in `workdir`. `Err` only when the repository can't be opened.
pub fn push(workdir: &Path, targets: &[PushTarget]) -> Result<StackPush> {
    let repo = Repository::open(workdir)?;
    let plans: Vec<PushPlan> = targets.iter().map(|t| classify(&repo, t)).collect();
    drop(repo);
    let mut outcomes: Vec<LayerOutcome> = plans
        .iter()
        .map(|&p| match p {
            PushPlan::UpToDate => LayerOutcome::UpToDate,
            p if p.pushes() => LayerOutcome::Failed,
            p => LayerOutcome::NotPushed(p),
        })
        .collect();
    let layers: Vec<(usize, &PushTarget, PushPlan)> = targets.iter().zip(&plans).enumerate().map(|(i, (t, p))| (i, t, *p)).collect();
    let mut output = String::new();
    for (cmd, indices) in porcelain::commands(&layers) {
        let out = gitcli::output(workdir, &cmd, "", &[], None);
        let shown = crate::text::redact_credentials(&cmd.join(" "));
        join(&mut output, &format!("$ git {shown}\n{}", out.combined()));
        for (flag, to, summary) in porcelain::parse(&out.stdout) {
            let Some(&i) = indices.iter().find(|&&i| to == format!("refs/heads/{}", targets[i].dest)) else { continue };
            outcomes[i] = match flag {
                ' ' | '+' | '*' => LayerOutcome::Pushed { plan: plans[i], upstream: targets[i].set_upstream },
                '=' => LayerOutcome::UpToDate,
                '!' if summary.contains("stale info") => LayerOutcome::Stale,
                '!' => LayerOutcome::Rejected(summary.replace(['[', ']'], "")),
                _ => continue,
            };
        }
    }
    let layers = targets.iter().zip(outcomes).map(|(t, outcome)| LayerPushed { target: t.clone(), outcome }).collect();
    Ok(StackPush { layers, output })
}

fn join(output: &mut String, more: &str) {
    let more = more.trim_end();
    if more.is_empty() {
        return;
    }
    if !output.is_empty() {
        output.push('\n');
    }
    output.push_str(more);
}

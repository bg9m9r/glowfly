//! Creating one pull request per layer of a stack: each layer's target (the branch below it,
//! by its name on the code host; the bottom layer's base), why the run can't start (the same
//! text for the branch menu, the palette and `update`), and the run itself, which
//! `update/stack_prs.rs` drives through the ordinary create dialog.

use super::push_pr;
use super::stack_push::{layers, remote_tip};
use super::{ForgeState, rebase_onto};
use std::collections::VecDeque;
use trellis_core::i18n::Lang;
use trellis_core::repo::Snapshot;

/// One layer's create dialog: the local branch, the name the code host knows it by, and the
/// target to preselect.
#[derive(Clone, Debug, PartialEq)]
pub struct PrLayer {
    pub branch: String,
    pub source: String,
    pub target: Option<String>,
}

/// A run of create dialogs: the layers still to come, the one whose dialog is open (`sent` once
/// its Create went to the service), its 1-based `index` of `total`, and what became of the rest.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StackPrRun {
    pub left: VecDeque<PrLayer>,
    pub current: Option<PrLayer>,
    pub sent: bool,
    pub index: usize,
    pub total: usize,
    pub created: Vec<String>,
    pub skipped: Vec<String>,
}

impl StackPrRun {
    /// (index, total) when the open dialog is the current layer's (by its source on the host).
    pub fn step_of(&self, source: &str) -> Option<(usize, usize)> {
        self.current.as_ref().filter(|c| c.source == source).map(|_| (self.index, self.total))
    }

    /// The layers that won't get a pull request from this run: the current one and those left.
    pub fn not_created(&self) -> Vec<String> {
        self.current.iter().chain(&self.left).map(|l| l.branch.clone()).collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum StackPrs {
    /// The push for the stack of this branch started; the dialogs start once the refreshed
    /// snapshot lands.
    AfterPush(String),
    Running(StackPrRun),
}

/// Every layer of the stack `branch` is in, bottom first, with its target: the branch below,
/// by its name on the code host, and for the bottom layer the stack's base.
pub fn pr_layers(snap: &Snapshot, fs: &ForgeState, branch: &str) -> Vec<PrLayer> {
    layers(&snap.stacks, branch)
        .into_iter()
        .map(|b| {
            let target = match snap.stacks.below(&b) {
                Some(l) => Some(push_pr::pr_source(snap, fs, &l.on)),
                None => base_for(snap, fs, &b),
            };
            PrLayer { source: push_pr::pr_source(snap, fs, &b), branch: b, target }
        })
        .collect()
}

/// The base the bottom layer `bottom` is built on, among the code host's default branch, the
/// main branch and Git Flow's development and production branches that are on the host's
/// remote: the one with the fewest commits between it and `bottom` (the first on a tie).
pub fn base_for(snap: &Snapshot, fs: &ForgeState, bottom: &str) -> Option<String> {
    let mut names: Vec<String> = fs.default_branch.iter().cloned().chain(snap.mainline()).collect();
    if let Some(flow) = snap.gitflow.settings() {
        names.extend([flow.development, flow.production]);
    }
    let mut candidates: Vec<(String, git2::Oid)> = Vec::new();
    for n in names {
        if !n.is_empty()
            && !candidates.iter().any(|(c, _)| *c == n)
            && let Some(o) = remote_tip(snap, &fs.remote, &n)
        {
            candidates.push((n, o));
        }
    }
    let Some(tip) = snap.local_oid(bottom) else { return candidates.into_iter().next().map(|(n, _)| n) };
    let mut best: Option<(usize, String)> = None;
    for (name, oid) in candidates {
        let n = snap.commits_between(tip, Some(oid)).len();
        if best.as_ref().is_none_or(|(b, _)| n < *b) {
            best = Some((n, name));
        }
    }
    best.map(|(_, n)| n)
}

/// Why the pull requests of the stack `branch` is in can't be created now: no code host, an
/// operation in progress, a layer that needs a restack first (its pull request would show the
/// lower layer's old commits), or every layer already has an open one.
pub fn prs_blocker(t: Lang, snap: &Snapshot, fs: &ForgeState, branch: &str) -> Option<String> {
    if fs.host.is_none() {
        let remote = snap.locals.iter().find(|b| b.name == branch).and_then(|b| push_pr::pr_remote(b, snap));
        return Some(push_pr::no_host_reason(remote.as_deref()));
    }
    if let Some(why) = rebase_onto::in_progress(t, snap.state) {
        return Some(why.to_owned());
    }
    let all = layers(&snap.stacks, branch);
    if let Some(l) = all.iter().find_map(|b| snap.stacks.below(b).filter(|l| l.behind)) {
        return Some(t.trf("stack.blocked.needs_restack", &[("branch", &l.branch), ("lower", &l.on)]));
    }
    let every_open = !all.is_empty() && all.iter().all(|b| push_pr::open_pr(snap, fs, b).is_some());
    every_open.then(|| t.trf("stack.blocked.prs_open", &[("noun", &fs.pr_word(t))]))
}

#[cfg(test)]
#[path = "stack_prs_tests.rs"]
mod tests;

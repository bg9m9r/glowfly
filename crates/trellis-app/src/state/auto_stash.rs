//! Stash and carry on: the auto-stash prompt (`Modal::AutoStash`), the stashes Trellis made
//! before an operation and still has to put back (`AppState::auto_stash`, global and never
//! parked), and the pure rules and texts around them. The flow is in update/auto_stash.rs.

use super::AppState;
use super::rebase::{has_conflicts, needs_stash};
use crate::msg::Msg;
use git2::{Oid, RepositoryState};
use std::path::PathBuf;
use trellis_core::i18n::Lang;
use trellis_core::paths::same_spelling;
use trellis_core::repo::Snapshot;
use trellis_core::stash_ops::{AutoStashFor, split_message};

/// The prompt: the operation, how many files changed, the Always stash box and the message that
/// starts the operation again once answered.
pub struct AutoStashAsk {
    pub op: AutoStashFor,
    pub files: usize,
    pub always: bool,
    pub retry: Box<Msg>,
}

/// The prompt's answer while its retry runs: `stash` false is Discard and reset.
#[derive(Clone, Debug, PartialEq)]
pub struct Approval {
    pub op: AutoStashFor,
    pub stash: bool,
}

/// A job that starts with an auto-stash: its operation, the stash's message and the stash
/// commits there were when it was built (the new entry is the one not among them).
#[derive(Clone, Debug, PartialEq)]
pub struct AutoStashJob {
    pub op: AutoStashFor,
    pub message: String,
    pub before: Vec<Oid>,
}

/// The auto-stash job that is running (or held for the trust prompt), matched by label and
/// workdir when a job ends.
#[derive(Clone, Debug, PartialEq)]
pub struct Making {
    pub label: String,
    pub workdir: Option<PathBuf>,
    pub job: AutoStashJob,
}

/// The operation a guarded job ran without stashing, and the message that starts it again:
/// what its failure's Stash and <op> sends.
pub struct StashRetry {
    pub op: AutoStashFor,
    pub retry: Box<Msg>,
}

/// The guarded job that runs (or waits for the trust prompt) without an auto-stash, matched by
/// label and workdir when a job ends.
pub struct Unstashed {
    pub label: String,
    pub workdir: Option<PathBuf>,
    pub retry: StashRetry,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// The job ended; only snapshots of load generation `from` or later may put it back.
    Ended { from: u64 },
    /// The put-back job runs.
    PuttingBack,
}

/// An auto-stash waiting to be put back in `workdir`'s tab.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingStash {
    pub workdir: PathBuf,
    pub job: AutoStashJob,
    /// The stash's commit, once a snapshot shows it.
    pub oid: Option<Oid>,
    pub phase: Phase,
    /// The "they come back when you continue or abort" toast was shown.
    pub told: bool,
}

#[derive(Default)]
pub struct AutoStashes {
    pub making: Option<Making>,
    pub pending: Vec<PendingStash>,
    /// Set only while the prompt's (or a failure toast's) retry message runs.
    pub approved: Option<Approval>,
    pub unstashed: Option<Unstashed>,
}

impl AutoStashes {
    /// The entry for `workdir`'s tab.
    pub fn index_of(&self, workdir: &std::path::Path) -> Option<usize> {
        self.pending.iter().position(|p| same_spelling(&p.workdir, workdir))
    }
}

/// How many files the stash would take: the distinct paths staged or changed (untracked files
/// stay).
pub fn changed_files(snap: &Snapshot) -> usize {
    let mut paths: Vec<&str> =
        snap.staged.iter().chain(snap.unstaged.iter().filter(|f| f.status != '?')).map(|f| f.path.as_str()).collect();
    paths.sort_unstable();
    paths.dedup();
    paths.len()
}

/// Whether the uncommitted changes call for the guard: nothing in progress, something to
/// stash, and no conflicted files (git stash would refuse those).
pub fn blocks(snap: &Snapshot) -> bool {
    snap.state == RepositoryState::Clean && needs_stash(snap) && !has_conflicts(snap)
}

/// The stash `job` made, among the entries that weren't there when it was built.
pub fn found(snap: &Snapshot, job: &AutoStashJob) -> Option<Oid> {
    snap.stashes.iter().find(|s| split_message(&s.message).1 == job.message && !job.before.contains(&s.oid)).map(|s| s.oid)
}

/// Whether the stash can go back: nothing in progress and nothing conflicted.
pub fn ready(snap: &Snapshot) -> bool {
    snap.state == RepositoryState::Clean && !has_conflicts(snap)
}

pub fn title(t: Lang) -> &'static str {
    t.tr("auto_stash.title")
}

/// The prompt's text: how many files, what Trellis does, and for an operation that can stop,
/// when the changes come back.
pub fn body(t: Lang, op: &AutoStashFor, files: usize) -> String {
    let what = match op {
        AutoStashFor::Checkout { target } => t.trf("auto_stash.body.checkout", &[("target", target)]),
        AutoStashFor::Pull { branch } => t.trf("auto_stash.body.pull", &[("branch", branch)]),
        AutoStashFor::Merge { from, into } => t.trf("auto_stash.body.merge", &[("from", from), ("into", into)]),
        AutoStashFor::Rebase { branch, onto } => t.trf("auto_stash.body.rebase", &[("branch", branch), ("onto", onto)]),
        AutoStashFor::CherryPick { commits, onto } => t.trf("auto_stash.body.cherry_pick", &[("commits", commits), ("onto", onto)]),
        AutoStashFor::Revert { commits, on } => t.trf("auto_stash.body.revert", &[("commits", commits), ("branch", on)]),
        AutoStashFor::HardReset { branch, to } => t.trf("auto_stash.body.reset", &[("branch", branch), ("commit", to)]),
    };
    let mut text = format!("{} {what}", t.trn("auto_stash.files", files, &[]));
    if op.can_stop() {
        text.push(' ');
        text.push_str(t.tr("auto_stash.body.stops"));
    }
    text
}

/// The primary: "Stash and <operation>".
pub fn button(t: Lang, op: &AutoStashFor) -> &'static str {
    match op {
        AutoStashFor::Checkout { .. } => t.tr("auto_stash.button.checkout"),
        AutoStashFor::Pull { .. } => t.tr("auto_stash.button.pull"),
        AutoStashFor::Merge { .. } => t.tr("auto_stash.button.merge"),
        AutoStashFor::Rebase { .. } => t.tr("auto_stash.button.rebase"),
        AutoStashFor::CherryPick { .. } => t.tr("auto_stash.button.cherry_pick"),
        AutoStashFor::Revert { .. } => t.tr("auto_stash.button.revert"),
        AutoStashFor::HardReset { .. } => t.tr("auto_stash.button.reset"),
    }
}

/// The hard reset's danger alternative.
pub fn discard_button(t: Lang) -> &'static str {
    t.tr("auto_stash.discard.button")
}

/// The WIP banner's line while an operation that stopped holds the active tab's auto-stash.
pub fn banner_note(st: &AppState) -> Option<String> {
    let snap = st.snap.as_ref()?;
    if snap.state == RepositoryState::Clean {
        return None;
    }
    let p = &st.auto_stash.pending[st.auto_stash.index_of(&snap.workdir)?];
    p.oid?;
    Some(st.settings.lang().trf("details.banner.auto_stash", &[("name", &p.job.message)]))
}

/// The toast when an operation stopped with the changes in the stash `name`.
pub fn made(t: Lang, name: &str) -> String {
    t.trf("auto_stash.toast.made", &[("name", &name)])
}

pub fn back(t: Lang, name: &str) -> String {
    t.trf("auto_stash.toast.back", &[("name", &name)])
}

pub fn conflicted(t: Lang, name: &str, files: usize) -> String {
    t.trn("auto_stash.toast.conflict", files, &[("name", &name)])
}

/// `reason` is the last line of git's output.
pub fn failed(t: Lang, name: &str, reason: &str) -> String {
    t.trf("auto_stash.toast.failed", &[("name", &name), ("reason", &reason)])
}

pub fn gone(t: Lang, name: &str) -> String {
    t.trf("auto_stash.toast.gone", &[("name", &name)])
}

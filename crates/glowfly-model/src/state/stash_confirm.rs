//! Whether applying, popping or deleting a stash asks first, the one place that decides it for
//! the STASHES menu, the palette, the toolbar's Pop and the journal's lost stashes. Pure.
//! state/stash_actions.rs is the entry point for a listed stash.
//!
//! Delete always asks. Apply and Pop are refused while the index has conflicted entries (git
//! can't apply a stash then), run straight away on a clean tree, and on top of uncommitted
//! changes ask a question that names the files both the stash and the working tree change. That
//! list needs the stash's files, so the question goes through `StashMsg::Ask`, which reads them
//! (`StashEffect::ReadPaths`) and opens it with `ask_confirm`. Every pop and apply of a listed
//! stash names it by position and commit (`StashMsg::Run`), so git is refused if the list moved.

use super::conflict_check::{POPOVER_FILES, file_list};
use super::rebase::has_conflicts;
use super::stashes::drop_confirm_body;
use crate::msg::{JobMsg, Msg, StashMsg};
use git2::Oid;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::Snapshot;
use std::collections::HashSet;

/// What Apply, Unstash and Delete run on one listed stash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StashAction {
    Apply,
    Pop,
    Drop,
}

impl StashAction {
    /// The `git stash` subcommand.
    pub fn git_verb(self) -> &'static str {
        match self {
            StashAction::Apply => "apply",
            StashAction::Pop => "pop",
            StashAction::Drop => "drop",
        }
    }

    /// The job's label in the activity log, as the sidebar names it.
    pub fn job_label(self) -> &'static str {
        match self {
            StashAction::Apply => "Apply stash",
            StashAction::Pop => "Unstash",
            StashAction::Drop => "Delete stash",
        }
    }
}

/// What an accepted Apply or Pop runs.
#[derive(Clone, Debug, PartialEq)]
pub enum StashRun {
    /// `git stash <verb> stash@{index}`, refused unless that still is the stash's commit.
    Listed { index: usize },
    /// A plain job, for a stash that isn't in the list (the journal's lost stashes).
    Git { label: String, cmds: Vec<Vec<String>> },
}

/// An Apply or Pop of the stash with commit `oid` (named `message`) on top of uncommitted
/// changes, waiting for the stash's files before it asks.
#[derive(Clone, Debug, PartialEq)]
pub struct StashAsk {
    pub action: StashAction,
    pub oid: Oid,
    pub message: String,
    pub run: StashRun,
}

impl StashAsk {
    /// The job the question's button runs.
    pub(super) fn job(&self) -> Msg {
        match &self.run {
            StashRun::Listed { index } => Msg::Stash(StashMsg::Run { action: self.action, index: *index, oid: self.oid }),
            StashRun::Git { label, cmds } => Msg::Job(JobMsg::Git { label: label.clone(), cmds: cmds.clone() }),
        }
    }
}

/// Why Apply and Pop can't run now, when the index has conflicted entries.
pub(crate) fn apply_blocked(snap: &Snapshot, t: Lang) -> Option<&'static str> {
    has_conflicts(snap).then(|| t.tr("stash.blocked.conflicts"))
}

/// What `action` on the stash with commit `oid` (named `message`) sends: the job, the question
/// before it, or the refusal over conflicts.
pub(super) fn guarded(snap: &Snapshot, t: Lang, action: StashAction, oid: Oid, message: &str, run: StashRun) -> Msg {
    let ask = StashAsk { action, oid, message: message.to_owned(), run };
    match action {
        StashAction::Drop => Msg::confirm_danger("Delete stash", drop_confirm_body(message), "Delete", ask.job()),
        StashAction::Apply | StashAction::Pop => match apply_blocked(snap, t) {
            Some(why) => Msg::Logged { ok: false, text: why.to_owned() },
            None if snap.is_dirty() => Msg::Stash(StashMsg::Ask(ask)),
            None => ask.job(),
        },
    }
}

/// The paths in `stash_paths` that the working tree changes too (staged, unstaged or
/// untracked, a staged rename's old path included), sorted.
pub(crate) fn overlap(stash_paths: &[String], snap: &Snapshot) -> Vec<String> {
    let changed: HashSet<&str> =
        snap.staged.iter().chain(&snap.unstaged).flat_map(|f| std::iter::once(f.path.as_str()).chain(f.renamed_from.as_deref())).collect();
    let mut both: Vec<String> = stash_paths.iter().filter(|p| changed.contains(p.as_str())).cloned().collect();
    both.sort();
    both.dedup();
    both
}

/// The question before applying or popping the stash named `message` on a dirty tree. `both`
/// holds the files the stash and the working tree both change; None when the stash's files
/// couldn't be read, which keeps the general warning.
pub(crate) fn dirty_body(t: Lang, action: StashAction, message: &str, both: Option<&[String]>) -> String {
    let mut body = t.trf("stash.ask.body", &[("message", &message)]);
    match both {
        None => {
            body.push(' ');
            body.push_str(t.tr("stash.ask.unknown"));
        }
        Some([]) => {
            body.push(' ');
            body.push_str(t.tr("stash.ask.apart"));
        }
        Some(paths) => {
            body.push(' ');
            body.push_str(&t.trn("stash.ask.overlap", paths.len(), &[]));
            let (shown, more) = file_list(t, paths, Some(POPOVER_FILES));
            for p in shown.iter().chain(&more) {
                body.push_str("\n    ");
                body.push_str(p);
            }
        }
    }
    if action == StashAction::Pop {
        body.push_str(if matches!(both, Some(p) if !p.is_empty()) { "\n\n" } else { " " });
        body.push_str(t.tr("stash.ask.pop_kept"));
    }
    body
}

/// The question for `ask`, given the stash's files (`paths`, or None when they couldn't be read).
pub fn ask_confirm(snap: &Snapshot, t: Lang, ask: &StashAsk, paths: Option<&[String]>) -> Msg {
    let both = paths.map(|p| overlap(p, snap));
    let button = if ask.action == StashAction::Pop { "Unstash" } else { "Apply" };
    Msg::confirm(ask.action.job_label(), dirty_body(t, ask.action, &ask.message, both.as_deref()), button, ask.job())
}

#[cfg(test)]
#[path = "stash_confirm_tests.rs"]
pub(super) mod tests;

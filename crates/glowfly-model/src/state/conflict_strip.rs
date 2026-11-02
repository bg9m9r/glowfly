//! The strip under the toolbar while the conflict view is open: the operation in words from the
//! open file's side labels ("Merging theme-tweaks into main"), how many conflicted files are
//! left, and the operation's Abort (quiet) and Commit merge or Continue (primary), which waits,
//! with why, until no file is conflicted. Elsewhere the strip is `op_strip`'s.

use super::AppState;
use super::am_banner::AM_CONTINUE;
use super::commit::commit_merge_blocked;
use super::op_actions::{action_text, state_actions};
use super::op_confirm::Risky;
use super::op_strip::strip_text;
use super::rebase_banner::action_msg;
use crate::msg::{CommitMsg, Msg};
use git2::RepositoryState;
use glowfly_core::i18n::Lang;
use glowfly_core::repo::{Operation, SideLabels};

/// The strip's text and buttons.
#[derive(Debug)]
pub struct ConflictStrip {
    /// "Merging theme-tweaks into main"; empty when nothing is in progress (conflicts alone).
    pub title: String,
    /// "1 conflicted file left", or "All conflicts resolved".
    pub detail: String,
    /// Files still conflicted.
    pub conflicted: usize,
    /// Abort's label and command, for `op_confirm::banner_msg`.
    pub abort: Option<(&'static str, &'static [&'static str])>,
    /// Commit merge or Continue.
    pub finish: Option<Finish>,
}

/// The primary button: its text, why it can't run now, and its command (`msg`).
#[derive(Debug)]
pub struct Finish {
    pub label: String,
    pub why: Option<String>,
    /// The operation's Continue (its English label and arguments); None for Commit merge.
    cont: Option<(RepositoryState, &'static str, &'static [&'static str])>,
}

impl Finish {
    /// What a click sends.
    pub fn msg(&self) -> Msg {
        match self.cont {
            None => Msg::Commit(CommitMsg::CommitMerge),
            Some((state, label, args)) => action_msg(state, label, args),
        }
    }
}

/// The operation in words, from the open file's side labels.
fn operation_text(t: Lang, l: &SideLabels) -> String {
    let sides: [(&str, &dyn std::fmt::Display); 2] = [("ours", &l.ours), ("theirs", &l.theirs)];
    match l.operation {
        Operation::Merge => t.trf("conflict.banner.operation.merge", &sides),
        Operation::Rebase => t.trf("conflict.banner.operation.rebase", &sides),
        Operation::CherryPick => t.trf("conflict.banner.operation.cherry_pick", &sides),
        Operation::Revert => t.trf("conflict.banner.operation.revert", &sides),
        Operation::Other => t.trf("conflict.banner.operation.applying", &sides),
    }
}

/// The strip while the conflict view is open; None when it isn't, or nothing is in progress or
/// conflicted.
pub fn conflict_strip(st: &AppState) -> Option<ConflictStrip> {
    let view = st.session.conflict.view.as_ref()?;
    let snap = st.snap.as_ref()?;
    let t = st.settings.lang();
    let text = strip_text(t, snap, st.session.conflict_files.resolved_rows(snap).len())?;
    let conflicted = snap.unstaged.iter().filter(|f| f.status == '!').count();
    let title = match (&view.file, snap.state) {
        (_, RepositoryState::Clean) => String::new(),
        (Some(f), _) => operation_text(t, &f.labels),
        (None, _) => text.title,
    };
    let detail = match conflicted {
        0 => t.tr("op_strip.all_resolved").to_owned(),
        n => t.trn("op_strip.conflicted", n, &[]),
    };
    let actions = state_actions(snap.state).unwrap_or_default();
    let abort = actions.iter().find(|(_, a)| Risky::of(a) == Some(Risky::Abort)).copied();
    let finish = finish(st, t, snap.state, conflicted);
    Some(ConflictStrip { title, detail, conflicted, abort: abort.filter(|_| finish.is_some()), finish })
}

/// Commit merge for a merge; Continue (named for the operation) for a rebase, cherry-pick,
/// revert or patch series; nothing for the others.
fn finish(st: &AppState, t: Lang, state: RepositoryState, conflicted: usize) -> Option<Finish> {
    use RepositoryState::*;
    let waiting = (conflicted > 0).then(|| t.trn("conflict.banner.resolve_first", conflicted, &[]));
    if state == Merge {
        let why = waiting.or_else(|| commit_merge_blocked(st));
        let label = action_text(t, "Commit merge").to_owned();
        return Some(Finish { label, why, cont: None });
    }
    let label = match state {
        Rebase | RebaseMerge | RebaseInteractive => t.tr("conflict.banner.continue_rebase.button"),
        CherryPick | CherryPickSequence => t.tr("conflict.banner.continue_cherry_pick.button"),
        Revert | RevertSequence => t.tr("conflict.banner.continue_revert.button"),
        ApplyMailbox => action_text(t, AM_CONTINUE),
        _ => return None,
    };
    let (name, args) = *state_actions(state)?.iter().find(|(l, _)| *l == "Continue")?;
    let why = waiting.or_else(|| st.job.is_some().then(|| t.tr("commit.blocked.job_running").to_owned()));
    Some(Finish { label: label.to_owned(), why, cont: Some((state, name, args)) })
}

#[cfg(test)]
#[path = "conflict_strip_tests.rs"]
mod tests;

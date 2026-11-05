//! The job's git work (`Effect::Job`, runner/job.rs): running git commands with the undo record
//! and the journal note, and undoing or redoing an action. Unscoped: the job's result must reach
//! `update` whichever tab is active.

use super::EffectMeta;
use super::SandboxRule;
use super::UndoOpts;
use super::partial::StopKey;
use crate::state::auto_stash::AutoStashJob;
use glowfly_core::undo::{Action, UndoKind};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum JobEffect {
    /// Runs git commands in order, stopping at the first failure. `write_file` is written first
    /// (commit messages, already in the repository's commit encoding). With `record`, the state
    /// before and after is compared and the resulting undoable action reported back; with
    /// `journal`, a journal note is written when the run moved HEAD or the op's branch
    /// (runner/journal.rs). Without `prompts` (background jobs) git fails rather than open a
    /// credential prompt.
    RunGit {
        cwd: PathBuf,
        cmds: Vec<Vec<String>>,
        write_file: Option<(PathBuf, Vec<u8>)>,
        record: Option<UndoKind>,
        journal: Option<glowfly_core::journal::Op>,
        prompts: bool,
        /// Run nothing unless this revision still resolves to this commit.
        expect: Option<(String, git2::Oid)>,
        /// Every command only fetches: `ControlEffect::StopGit(StopKey::Job)` can stop it.
        stoppable: bool,
        /// Run after a failure: when this check passes the job ends with
        /// `JobMsg::FailCheckPassed` instead of `JobDone` (a patch that is already applied).
        on_fail_check: Option<Vec<String>>,
        /// The auto-stash its first command makes: once that command succeeds the runner writes
        /// its saved record and sends `AutoStashMsg::Made` (runner/auto_stash_made.rs).
        auto_stash: Option<AutoStashJob>,
    },
    /// Undoes (or redoes) `action` after checking the repository still matches it.
    UndoRedo { cwd: PathBuf, action: Action, redo: bool, opts: UndoOpts },
}

impl EffectMeta for JobEffect {
    fn name(&self) -> &'static str {
        match self {
            JobEffect::RunGit { .. } => "git",
            JobEffect::UndoRedo { .. } => "undo-redo",
        }
    }

    fn scope(&self) -> Option<&PathBuf> {
        None
    }

    fn timed(&self) -> bool {
        true
    }

    /// A job waits for the repository's other changes, unless every command only fetches or
    /// pushes.
    fn queue(&self) -> Option<&Path> {
        match self {
            JobEffect::RunGit { cmds, .. } if cmds.iter().all(|c| network_only(c)) => None,
            JobEffect::RunGit { cwd, .. } | JobEffect::UndoRedo { cwd, .. } => Some(cwd),
        }
    }

    /// A git job holds its repository even when it only fetches or pushes: git may repack after.
    fn holds(&self) -> Option<&Path> {
        match self {
            JobEffect::RunGit { cwd, .. } => Some(cwd),
            JobEffect::UndoRedo { .. } => self.queue(),
        }
    }

    fn stop_key(&self) -> Option<StopKey> {
        match self {
            JobEffect::RunGit { stoppable: true, .. } => Some(StopKey::Job),
            JobEffect::RunGit { .. } | JobEffect::UndoRedo { .. } => None,
        }
    }

    fn sandbox(&self) -> SandboxRule {
        SandboxRule::Run
    }
}

/// Fetch and push update remote-tracking refs at most; they never touch the index or files.
fn network_only(cmd: &[String]) -> bool {
    matches!(glowfly_core::gitcli::subcommand(cmd), Some("fetch" | "push")) && !cmd.iter().any(|a| a == ".")
}

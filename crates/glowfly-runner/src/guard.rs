//! Keeps a panicking effect from leaving the UI waiting forever: the panic is caught and turned
//! into the failure result the effect's sender is waiting for (a finished job clears `st.job`).

use crate::effect::partial::PartialEffect;
use crate::effect::{
    CloneEffect, Effect, FailedLineCounts, ForgeError, InitEffect, LoadKind, RebaseEffect, ReposEffect, SparseEffect, StashEffect,
    SubmoduleEffect, UndoResult, UpdateEffect, WorktreeEffect,
};
use crate::msg::BlameMsg;
use crate::msg::CloneMsg;
use crate::msg::JobMsg;
use crate::msg::LoadMsg;
use crate::msg::Msg;
use crate::msg::RebaseMsg;
use crate::msg::RepoMsg;
use crate::msg::StageMsg;
use crate::msg::UpdateMsg;
use crate::msg::{ContentFailure, PartialMsg};
use crate::patch::{PatchEffect, PatchMsg};
use crate::remote::{RemoteEffect, RemoteMsg};
use glowfly_core::forge::{Account, Target};
use glowfly_model::ssh::SshFailure;
use glowfly_model::tools::ToolEffect;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

/// What to report if an effect panics, worked out before it runs.
pub(super) struct Recovery {
    name: &'static str,
    scope: Option<PathBuf>,
    kind: Kind,
}

enum Kind {
    /// A job that holds `st.job`: it must finish, failed, with this undo outcome.
    Job(Option<UndoResult>),
    Rebase,
    /// Editing a running rebase's remaining steps: the panel waits for it.
    EditTodo,
    LocalOp,
    /// A repository load or status read: the UI waits for it before starting another.
    Load(u64, LoadKind),
    Status(u64),
    /// Preferences > SSH work (reading ~/.ssh or the agent, generating or adding a key, writing
    /// core.sshCommand): the page shows a spinner until its result arrives.
    Ssh(SshFailure),
    /// A fork dialog's request: the dialog shows a spinner until its answer arrives.
    Fork(FailedFork),
    /// The Clone dialog's list of the user's repositories on this account: a spinner until it ends.
    Repos(Account),
    /// The blame view shows "Loading…" until its result arrives.
    Blame {
        path: String,
        generation: u64,
    },
    /// An update check (`Some(manual)`) or download (None): the status shows a spinner until
    /// its result arrives.
    Update(Option<bool>),
    /// Install and restart: `restarting` holds until its answer arrives.
    UpdateRestart,
    /// A scan of the project folders: the Find repositories dialog shows a spinner until it ends.
    RepoScan(u64, Vec<PathBuf>),
    /// One repository of a fetch or pull across repositories (generation, row): it fails, so
    /// the next one starts.
    RepoSync(u64, usize),
    /// A commit's or the working tree's line counts: pending until a result arrives.
    LineCounts(FailedLineCounts),
    /// Putting an auto-stash back (workdir, stash): it ends the active job.
    PutBack(PathBuf, git2::Oid),
    /// Fetch full history's size check (workdir, id): the status bar shows it until it ends.
    Sizing(PathBuf, u64),
    /// A background download of missing contents: its pane shows it until it ends.
    Content(u64),
    /// Saving a patch to this file: it says the patch wasn't created.
    Patch(PathBuf),
    Log,
}

enum FailedFork {
    List(PathBuf, Target),
    Find(PathBuf, Target),
    Create(PathBuf, Target, String),
}

impl FailedFork {
    fn of(e: &RemoteEffect) -> Self {
        match e {
            RemoteEffect::ListForks(r) => FailedFork::List(r.workdir.clone(), r.target.clone()),
            RemoteEffect::FindMyFork(r) => FailedFork::Find(r.workdir.clone(), r.target.clone()),
            RemoteEffect::CreateFork { req, name } => FailedFork::Create(req.workdir.clone(), req.target.clone(), name.clone()),
        }
    }

    fn message(self, text: String) -> Msg {
        let err = ForgeError { message: text, auth: false };
        Msg::Remote(match self {
            FailedFork::List(workdir, target) => RemoteMsg::ForksLoaded { workdir, target, result: Box::new(Err(err)) },
            FailedFork::Find(workdir, target) => RemoteMsg::MyForkFound { workdir, target, result: Box::new(Err(err)) },
            FailedFork::Create(workdir, target, name) => RemoteMsg::ForkCreated { workdir, target, name, result: Box::new(Err(err)) },
        })
    }
}

impl Recovery {
    pub(super) fn of(effect: &Effect, name: &'static str) -> Self {
        let kind = match effect {
            Effect::RunGit { record, .. } => Kind::Job(record.as_ref().map(|_| UndoResult::Recorded(None))),
            Effect::Pick(j) => Kind::Job(j.record.as_ref().map(|_| UndoResult::Recorded(None))),
            Effect::UndoRedo { redo, .. } => Kind::Job(Some(UndoResult::Failed { redo: *redo, stash: None })),
            Effect::Worktree(WorktreeEffect::Remove(_) | WorktreeEffect::Move(_))
            | Effect::Submodule(SubmoduleEffect::Remove(_))
            | Effect::Stash(StashEffect::Rename { .. })
            | Effect::Sparse(SparseEffect::Checkout { .. })
            | Effect::TurnOnMonitor(_)
            | Effect::TurnOffMonitor(_)
            | Effect::Stack(_)
            | Effect::Clone(CloneEffect::Sparse { .. })
            | Effect::Init(InitEffect::Run { .. })
            | Effect::Tool(ToolEffect::RunMerge(_) | ToolEffect::RunGitMerge(_)) => Kind::Job(None),
            Effect::Clone(CloneEffect::ListRepos { account, .. }) => Kind::Repos(account.clone()),
            Effect::Rebase(RebaseEffect::Start { .. } | RebaseEffect::Resume { .. }) => Kind::Rebase,
            Effect::Rebase(RebaseEffect::EditTodo { .. }) => Kind::EditTodo,
            Effect::LocalOp { .. } => Kind::LocalOp,
            Effect::LoadRepo { generation, kind, .. } => Kind::Load(*generation, *kind),
            Effect::LoadStatus { generation, .. } | Effect::LoadIndexStatus { generation, .. } => Kind::Status(*generation),
            Effect::Ssh(e) => Kind::Ssh(e.failure()),
            Effect::Remote(e) => Kind::Fork(FailedFork::of(e)),
            Effect::LoadBlame(r) => Kind::Blame { path: r.path.clone(), generation: r.generation },
            Effect::Updates(UpdateEffect::Check { manual }) => Kind::Update(Some(*manual)),
            Effect::Updates(UpdateEffect::Download(_)) => Kind::Update(None),
            Effect::Updates(UpdateEffect::Restart(_)) => Kind::UpdateRestart,
            Effect::Repos(ReposEffect::Scan { generation, roots }) => Kind::RepoScan(*generation, roots.clone()),
            Effect::Repos(ReposEffect::Sync { generation, index, .. }) => Kind::RepoSync(*generation, *index),
            Effect::LineCounts(e) => Kind::LineCounts(e.failure()),
            Effect::Stash(StashEffect::PutBack(r)) => Kind::PutBack(r.cwd.clone(), r.oid),
            Effect::Partial(PartialEffect::SizeHistory(r)) => Kind::Sizing(r.workdir.clone(), r.id),
            Effect::Partial(PartialEffect::FetchContent(r)) => Kind::Content(r.id),
            Effect::Patch(PatchEffect::Export { dest, .. }) => Kind::Patch(dest.clone()),
            _ => Kind::Log,
        };
        Self { name, scope: effect.scope().cloned(), kind }
    }

    fn message(self, panic: String) -> Msg {
        let text = format!("Glowfly hit an internal error ({}): {panic}", self.name);
        self.failed(text)
    }

    /// The failure message the effect's sender waits for, saying `text` (a trust re-check that
    /// couldn't be made, runner/trust.rs).
    pub(super) fn failed(self, text: String) -> Msg {
        let msg = match self.kind {
            Kind::Job(undo) => Msg::Job(JobMsg::JobDone { ok: false, output: text, undo }),
            Kind::Rebase => Msg::Rebase(RebaseMsg::RebaseFinished { result: Err(text), undo: None }),
            Kind::EditTodo => Msg::Rebase(RebaseMsg::RemainingEdited { result: Err(text) }),
            Kind::LocalOp => Msg::Stage(StageMsg::LocalOpDone(Err(text))),
            Kind::Load(generation, kind) => Msg::Repo(RepoMsg::RepoLoaded { generation, kind, result: Err(text) }),
            Kind::Status(generation) => {
                let result = Err(text);
                Msg::Load(LoadMsg::StatusLoaded {
                    generation,
                    result,
                    submodules: None,
                    open_stamp: None,
                    slow: None,
                    lfs_patterns: None,
                    index: None,
                })
            }
            Kind::Ssh(f) => Msg::Ssh(f.message(text)),
            Kind::Fork(f) => f.message(text),
            Kind::Repos(account) => {
                let result = Box::new(Err(ForgeError { message: text, auth: false }));
                Msg::Clone(CloneMsg::ReposLoaded { account, result })
            }
            Kind::Blame { path, generation } => Msg::Blame(BlameMsg::Loaded { path, generation, result: Err(text) }),
            Kind::Update(Some(manual)) => Msg::Updates(UpdateMsg::Checked { manual, result: Err(text) }),
            Kind::Update(None) => Msg::Updates(UpdateMsg::Downloaded(Err(text))),
            Kind::UpdateRestart => Msg::Updates(UpdateMsg::RestartReady(Err(text))),
            Kind::RepoScan(generation, roots) => {
                let unreadable = roots.into_iter().map(|r| (r, text.clone())).collect();
                let report = Box::new(crate::msg::ScanFound { unreadable, ..Default::default() });
                Msg::Repos(crate::msg::ReposMsg::Scanned { generation, report })
            }
            Kind::RepoSync(generation, index) => {
                let outcome = glowfly_core::repo_sync::Outcome::Failed { reason: text, detail: String::new() };
                Msg::Repos(crate::msg::ReposMsg::Synced { generation, index, outcome })
            }
            Kind::LineCounts(FailedLineCounts::Commit(oid)) => Msg::Repo(RepoMsg::CommitLineCountsLoaded { oid, result: Err(text) }),
            Kind::LineCounts(FailedLineCounts::Wip(generation)) => {
                Msg::Repo(RepoMsg::WipLineCountsLoaded { generation, result: Err(text) })
            }
            Kind::PutBack(workdir, oid) => {
                let outcome = glowfly_core::stash_ops::PutBack::Failed;
                Msg::AutoStash(crate::msg::AutoStashMsg::PutBack { workdir, oid, outcome, output: text, undo: None })
            }
            Kind::Sizing(workdir, id) => {
                tracing::debug!("{text}");
                Msg::Partial(PartialMsg::HistorySized { workdir, id, size: glowfly_core::history_size::HistorySize::Unknown })
            }
            Kind::Content(id) => Msg::Partial(PartialMsg::ContentFetched { id, result: Err(ContentFailure::Failed(text)) }),
            Kind::Patch(dest) => Msg::Patch(PatchMsg::Exported { dest, result: Err(text) }),
            Kind::Log => Msg::Logged { ok: false, text },
        };
        match self.scope {
            Some(workdir) => Msg::Scoped { workdir, msg: Box::new(msg) },
            None => msg,
        }
    }
}

/// Runs `f`, turning a panic into `recovery`'s failure message (and logging it).
pub(super) fn guarded(recovery: Recovery, f: impl FnOnce() -> Option<Msg>) -> Option<Msg> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(msg) => msg,
        Err(payload) => {
            let panic = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_owned());
            tracing::error!(effect = recovery.name, "effect panicked: {panic}");
            Some(recovery.message(panic))
        }
    }
}

#[cfg(test)]
#[path = "guard_tests.rs"]
mod tests;

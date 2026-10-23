//! One queue per repository for the effects that change it (index, working tree, refs), so a
//! stage, discard or conflict save never runs at the same time as a pull, a checkout or another
//! edit of the same repository: each runs after the ones queued before it. Reads (loading
//! status, diffs, history) and network-only jobs (fetch, push) don't queue.
//!
//! A queue belongs to a folder, not to a spelling of its path: jobs for `C:/x/repo/`, a junction
//! to it, or `REPO` on a volume that ignores case share one worker. Telling them apart takes the
//! disk (`paths::FolderId`), so `submit` hands each job to a router thread that looks the folder
//! up (once per spelling) and passes the job on, in order; the UI thread never waits on it.

use crate::custom_commands::CustomCommandEffect;
use crate::effect::{Effect, RebaseEffect, ReposEffect, SubmoduleEffect};
use glowfly_core::paths::FolderId;
use glowfly_core::repo_sync::SyncOp;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc::{self, Sender};

pub(super) type Job = Box<dyn FnOnce() + Send>;

/// What a queue is keyed by: the folder's identity, or (for a path with none: missing, or on a
/// volume without file ids) its resolved spelling.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Key {
    Folder(FolderId),
    Spelling(PathBuf),
}

/// Finds the key of a repository path; `identify` in production, a stand-in in tests.
type Identify = fn(&Path) -> Key;

fn identify(repo: &Path) -> Key {
    match FolderId::of(repo) {
        Some(id) => Key::Folder(id),
        None => Key::Spelling(glowfly_core::paths::resolve(repo).components().collect()),
    }
}

pub(super) struct RepoQueues {
    /// The router thread, started on first use.
    router: Mutex<Option<Sender<(PathBuf, Job)>>>,
    identify: Identify,
}

impl Default for RepoQueues {
    fn default() -> Self {
        Self { router: Mutex::new(None), identify }
    }
}

impl RepoQueues {
    /// Queues `job` behind the earlier jobs for the folder `repo` names. Returns the job back if
    /// no thread could be started for it.
    pub(super) fn submit(&self, repo: &Path, job: Job) -> Result<(), Job> {
        let mut router = self.router.lock().unwrap_or_else(|p| p.into_inner());
        let mut job = job;
        if let Some(tx) = router.as_ref() {
            match tx.send((repo.to_owned(), job)) {
                Ok(()) => return Ok(()),
                Err(mpsc::SendError((_, back))) => job = back, // the router is gone; start another
            }
        }
        let (tx, rx) = mpsc::channel::<(PathBuf, Job)>();
        let identify = self.identify;
        let started = std::thread::Builder::new().name("glowfly-repo-queues".into()).spawn(move || route(rx, identify));
        if started.is_err() {
            return Err(job);
        }
        // The router is alive, so the send can't fail.
        let _ = tx.send((repo.to_owned(), job));
        *router = Some(tx);
        Ok(())
    }
}

/// Passes each job to its folder's worker, starting workers on first use. A job whose worker
/// can't be started runs on a thread of its own, or here as a last resort.
fn route(rx: mpsc::Receiver<(PathBuf, Job)>, identify: Identify) {
    let mut keys: HashMap<PathBuf, Key> = HashMap::new();
    let mut workers: HashMap<Key, Sender<Job>> = HashMap::new();
    for (repo, job) in rx {
        let key = keys.entry(repo.clone()).or_insert_with(|| identify(&repo)).clone();
        let mut job = job;
        if let Some(tx) = workers.get(&key) {
            match tx.send(job) {
                Ok(()) => continue,
                Err(mpsc::SendError(back)) => job = back, // its worker is gone; start another
            }
        }
        let (tx, worker_rx) = mpsc::channel::<Job>();
        let name = format!("glowfly-repo-{}", repo.file_name().map(|n| n.to_string_lossy()).unwrap_or_default());
        let started = std::thread::Builder::new().name(name).spawn(move || {
            for job in worker_rx {
                job();
            }
        });
        if started.is_err() {
            tracing::error!("could not start a queue for {}", repo.display());
            if let Err(e) = std::thread::Builder::new().name("glowfly-repo-job".into()).spawn(job) {
                tracing::error!("could not start a thread for a job: {e}");
            }
            continue;
        }
        // The worker is alive, so the send can't fail.
        let _ = tx.send(job);
        workers.insert(key, tx);
    }
}

/// The repository whose queue `effect` runs on, when it can change that repository.
pub(super) fn repo_of(effect: &Effect) -> Option<&Path> {
    match effect {
        Effect::LocalOp { workdir, .. }
        | Effect::Revert { workdir, .. }
        | Effect::RestoreFile { workdir, .. }
        | Effect::WorkFile { workdir, .. } => Some(workdir),
        Effect::RunGit { cmds, .. } if cmds.iter().all(|c| network_only(c)) => None,
        Effect::RunGit { cwd, .. } | Effect::UndoRedo { cwd, .. } => Some(cwd),
        Effect::Rebase(
            RebaseEffect::Start { cwd, .. }
            | RebaseEffect::Resume { cwd, .. }
            | RebaseEffect::EditTodo { cwd, .. }
            | RebaseEffect::Restack { cwd, .. }
            | RebaseEffect::ResumeRestack { cwd, .. },
        ) => Some(cwd),
        Effect::Pick(j) => Some(&j.cwd),
        Effect::RemoveWorktree(r) => Some(&r.repo),
        Effect::MoveWorktree(r) => Some(&r.repo),
        Effect::Submodule(SubmoduleEffect::Remove(r)) => Some(&r.repo),
        Effect::Stash(e) => Some(e.cwd()),
        Effect::SparseCheckout { cwd, .. } => Some(cwd),
        // It writes the repository's config and its index.
        Effect::TurnOnMonitor { cwd, .. } => Some(cwd),
        // A script may change the index, files and refs.
        Effect::CustomCommand(CustomCommandEffect::RunShell(r)) => Some(&r.cwd),
        Effect::Repos(ReposEffect::Sync { path, op: SyncOp::Pull, .. }) => Some(path),
        // Reads, network calls, Glowfly's own files (ordered by `saves`), git config (which no
        // queued job reads mid-way) and work outside the process. Listed one by one so a new
        // effect has to decide.
        Effect::LoadRepo { .. }
        | Effect::LoadStatus { .. }
        | Effect::LoadIndexStatus { .. }
        | Effect::CountWorktreeChanges(_)
        | Effect::LoadDetail { .. }
        | Effect::LoadCompare { .. }
        | Effect::LoadStacked(_)
        | Effect::LoadWorkdirCompare { .. }
        | Effect::LoadDiff { .. }
        | Effect::LoadDiffGaps { .. }
        | Effect::LoadBinarySides { .. }
        | Effect::HighlightDiff { .. }
        | Effect::LoadConflict { .. }
        | Effect::ReadConflictFiles { .. }
        | Effect::LoadFileHistory { .. }
        | Effect::LoadBlame(_)
        | Effect::LoadFileText(_)
        | Effect::LoadTrackedPaths { .. }
        | Effect::Submodule(
            SubmoduleEffect::Check { .. }
            | SubmoduleEffect::CheckAddPath { .. }
            | SubmoduleEffect::CheckRemove { .. }
            | SubmoduleEffect::CheckAll { .. },
        )
        | Effect::CheckWorktreeFolder(_)
        | Effect::CheckCloneParent { .. }
        | Effect::CheckRepoFolders(_)
        | Effect::ReadTabGroups(_)
        | Effect::DetectLfs { .. }
        | Effect::LoadLfs { .. }
        | Effect::LoadSparse { .. }
        | Effect::Rebase(RebaseEffect::LoadPlan { .. } | RebaseEffect::PrepareEdit { .. } | RebaseEffect::LoadRemaining { .. })
        | Effect::Forge { .. }
        | Effect::Account(_)
        | Effect::Stats { .. }
        | Effect::LineCounts(_)
        | Effect::SearchPaths { .. }
        | Effect::Index(_)
        | Effect::Background(_)
        | Effect::VerifySignature { .. }
        | Effect::FetchAvatars(_)
        | Effect::Prefs(_)
        | Effect::Profile(_)
        | Effect::Onboarding(_)
        | Effect::SaveSettings(_)
        | Effect::SaveRecent(_)
        | Effect::SaveTabs { .. }
        | Effect::Os(_)
        | Effect::Ssh(_)
        | Effect::Watch(_)
        | Effect::SweepWatch { .. }
        | Effect::AnswerPrompt { .. }
        | Effect::Credential(_)
        | Effect::Terminal(_)
        | Effect::Patch(_)
        | Effect::Tool(_)
        | Effect::CustomCommand(CustomCommandEffect::Load { .. })
        | Effect::GitEnv(_)
        | Effect::Theme(_)
        | Effect::CheckConflicts(_)
        | Effect::OpPreview(_)
        | Effect::Journal(_)
        | Effect::RangeDiff(_)
        | Effect::Remote(_)
        | Effect::Clone(_)
        | Effect::Init(_)
        | Effect::Trust(_)
        | Effect::Notify(_)
        | Effect::Logs(_)
        // Reads and calls, and a pull request head fetch that writes no ref.
        | Effect::PullRequest(_)
        | Effect::DeepLink(_)
        | Effect::Dashboard(_)
        | Effect::Issue(_)
        | Effect::Updates(_)
        | Effect::Repos(ReposEffect::Scan { .. } | ReposEffect::Sync { op: SyncOp::Fetch { .. }, .. })
        // A network call, and a fetch that only adds objects.
        | Effect::Partial(_)
        // Pushes update remote-tracking refs at most, like a push RunGit (`network_only`).
        | Effect::Stack(_)
        | Effect::StopGit(_)
        | Effect::Ui(_) => None,
    }
}

/// The repository `effect` keeps out of the repository pool while it runs (`repo_pool::hold`):
/// one it can change, and a fetch's or a push's too, after which git may repack. On Windows an
/// open handle keeps the old packs from being deleted.
pub(super) fn holds(effect: &Effect) -> Option<&Path> {
    repo_of(effect).or(match effect {
        Effect::RunGit { cwd, .. } => Some(cwd),
        Effect::Repos(ReposEffect::Sync { path, .. }) => Some(path),
        _ => None,
    })
}

/// Fetch and push update remote-tracking refs at most; they never touch the index or files.
fn network_only(cmd: &[String]) -> bool {
    matches!(glowfly_core::gitcli::subcommand(cmd), Some("fetch" | "push")) && !cmd.iter().any(|a| a == ".")
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;

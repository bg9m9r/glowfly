//! One queue per repository for the effects that change it (index, working tree, refs), so a
//! stage, discard or conflict save never runs at the same time as a pull, a checkout or another
//! edit of the same repository: each runs after the ones queued before it. Reads (loading
//! status, diffs, history) and network-only jobs (fetch, push) don't queue.

use crate::effect::Effect;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc::{self, Sender};

pub(super) type Job = Box<dyn FnOnce() + Send>;

#[derive(Default)]
pub(super) struct RepoQueues {
    /// A worker thread per repository, started on first use, that runs its jobs in order.
    workers: Mutex<HashMap<PathBuf, Sender<Job>>>,
}

impl RepoQueues {
    /// Queues `job` behind the earlier jobs for `repo`. Returns the job back if no worker
    /// thread could be started for it.
    pub(super) fn submit(&self, repo: &Path, job: Job) -> Result<(), Job> {
        let mut workers = self.workers.lock().unwrap_or_else(|p| p.into_inner());
        let mut job = job;
        if let Some(tx) = workers.get(repo) {
            match tx.send(job) {
                Ok(()) => return Ok(()),
                Err(mpsc::SendError(back)) => job = back, // its worker is gone; start another
            }
        }
        let (tx, rx) = mpsc::channel::<Job>();
        let name = format!("trellis-repo-{}", repo.file_name().map(|n| n.to_string_lossy()).unwrap_or_default());
        let started = std::thread::Builder::new().name(name).spawn(move || {
            for job in rx {
                job();
            }
        });
        if started.is_err() {
            return Err(job);
        }
        // The worker is alive, so the send can't fail.
        let _ = tx.send(job);
        workers.insert(repo.to_owned(), tx);
        Ok(())
    }
}

/// The repository whose queue `effect` runs on, when it can change that repository.
pub(super) fn repo_of(effect: &Effect) -> Option<&Path> {
    match effect {
        Effect::LocalOp { workdir, .. }
        | Effect::RevertHunk { workdir, .. }
        | Effect::RestoreFile { workdir, .. }
        | Effect::WorkFile { workdir, .. } => Some(workdir),
        Effect::RunGit { cmds, .. } if cmds.iter().all(|c| network_only(c)) => None,
        Effect::RunGit { cwd, .. } | Effect::UndoRedo { cwd, .. } | Effect::InteractiveRebase { cwd, .. } => Some(cwd),
        Effect::RemoveWorktree(r) => Some(&r.repo),
        Effect::RenameStash { cwd, .. } => Some(cwd),
        Effect::SparseCheckout { cwd, .. } => Some(cwd),
        // Reads, network calls, Trellis's own files (ordered by `saves`), git config (which no
        // queued job reads mid-way) and work outside the process. Listed one by one so a new
        // effect has to decide.
        Effect::LoadRepo { .. }
        | Effect::LoadStatus { .. }
        | Effect::LoadDetail { .. }
        | Effect::LoadCompare { .. }
        | Effect::LoadDiff { .. }
        | Effect::HighlightDiff { .. }
        | Effect::LoadConflict { .. }
        | Effect::LoadFileHistory { .. }
        | Effect::LoadTrackedPaths { .. }
        | Effect::CheckSubmodule { .. }
        | Effect::CheckWorktreeFolder(_)
        | Effect::CheckCloneParent { .. }
        | Effect::CheckRepoFolders(_)
        | Effect::DetectLfs { .. }
        | Effect::LoadLfs { .. }
        | Effect::LoadSparse { .. }
        | Effect::LoadRebasePlan { .. }
        | Effect::Forge { .. }
        | Effect::RepoOwners { .. }
        | Effect::CreateRepo { .. }
        | Effect::Connect { .. }
        | Effect::SaveToken { .. }
        | Effect::DeleteToken(_)
        | Effect::ForgetCredentialManager { .. }
        | Effect::Stats { .. }
        | Effect::VerifySignature { .. }
        | Effect::FetchAvatars(_)
        | Effect::ReadGitConfig { .. }
        | Effect::SetGitConfig { .. }
        | Effect::SaveSettings(_)
        | Effect::SaveRecent(_)
        | Effect::SaveTabs { .. }
        | Effect::OpenUrl(_)
        | Effect::OpenTerminal(_)
        | Effect::OpenFolder(_)
        | Effect::OpenPath(_)
        | Effect::RevealPath(_)
        | Effect::PickFolder { .. }
        | Effect::PickFile { .. }
        | Effect::ListSigningKeys { .. }
        | Effect::GenerateGpgKey { .. }
        | Effect::ExportPublicKey { .. }
        | Effect::Ssh(_)
        | Effect::Watch(_)
        | Effect::AnswerPrompt { .. }
        | Effect::Terminal(_)
        | Effect::Patch(_)
        | Effect::Tool(_)
        | Effect::CheckConflicts(_)
        | Effect::Ui(_) => None,
    }
}

/// Fetch and push update remote-tracking refs at most; they never touch the index or files.
fn network_only(cmd: &[String]) -> bool {
    matches!(cmd.first().map(String::as_str), Some("fetch" | "push")) && !cmd.iter().any(|a| a == ".")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn jobs_for_one_repository_run_one_at_a_time_in_order() {
        let q = RepoQueues::default();
        let (gate_tx, gate_rx) = mpsc::channel::<()>();
        let (done_tx, done_rx) = mpsc::channel::<&'static str>();
        let running = Arc::new(AtomicUsize::new(0));
        let a = Path::new("C:/repo-a");
        let (d, r) = (done_tx.clone(), running.clone());
        q.submit(
            a,
            Box::new(move || {
                r.fetch_add(1, Ordering::SeqCst);
                gate_rx.recv().unwrap(); // a long pull
                r.fetch_sub(1, Ordering::SeqCst);
                d.send("pull").unwrap();
            }),
        )
        .ok()
        .unwrap();
        let (d, r) = (done_tx.clone(), running.clone());
        q.submit(
            a,
            Box::new(move || {
                assert_eq!(r.load(Ordering::SeqCst), 0, "the pull has finished");
                d.send("stage").unwrap();
            }),
        )
        .ok()
        .unwrap();
        // Another repository's queue isn't held up.
        let d = done_tx.clone();
        q.submit(Path::new("C:/repo-b"), Box::new(move || d.send("other repo").unwrap())).ok().unwrap();
        assert_eq!(done_rx.recv_timeout(Duration::from_secs(10)).unwrap(), "other repo");
        assert!(done_rx.recv_timeout(Duration::from_millis(200)).is_err(), "the stage waits for the pull");
        gate_tx.send(()).unwrap();
        assert_eq!(done_rx.recv_timeout(Duration::from_secs(10)).unwrap(), "pull");
        assert_eq!(done_rx.recv_timeout(Duration::from_secs(10)).unwrap(), "stage");
    }

    #[test]
    fn which_effects_queue() {
        let w = PathBuf::from("C:/r");
        let git = |cmds: &[&[&str]]| Effect::RunGit {
            cwd: w.clone(),
            cmds: cmds.iter().map(|c| trellis_core::gitcli::args(c)).collect(),
            write_file: None,
            record: None,
            prompts: false,
            expect: None,
        };
        assert_eq!(repo_of(&git(&[&["pull"]])), Some(w.as_path()));
        assert_eq!(repo_of(&git(&[&["fetch", "--all"]])), None, "auto-fetch doesn't hold up staging");
        assert_eq!(repo_of(&git(&[&["fetch", ".", "a:b"]])), Some(w.as_path()));
        let op = Effect::LocalOp { workdir: w.clone(), op: crate::effect::LocalOp::StageAll };
        assert_eq!(repo_of(&op), Some(w.as_path()));
        assert_eq!(
            repo_of(&Effect::LoadStatus { workdir: w.clone(), generation: 0, submodules: false, open: None, git_status: true }),
            None
        );
        // Saving a patch only reads the repository.
        let export = crate::patch::PatchEffect::Export { workdir: w.clone(), oids: Vec::new(), dest: "C:/x.patch".into() };
        assert_eq!(repo_of(&Effect::Patch(export)), None);
        assert_eq!(repo_of(&Effect::Tool(crate::tools::ToolEffect::PickProgram(crate::tools::ToolKind::Editor))), None);
        // A conflict check writes only unreferenced objects.
        let request = crate::conflict_check::CheckRequest {
            branch: "feature".into(),
            ours: git2::Oid::zero(),
            target: trellis_core::conflict_check::MergeTarget {
                name: "main".into(),
                oid: git2::Oid::zero(),
                kind: trellis_core::conflict_check::TargetKind::DefaultBranch,
            },
        };
        let job = crate::conflict_check::CheckJob { workdir: w.clone(), request, manual: false, generation: 0 };
        assert_eq!(repo_of(&Effect::CheckConflicts(job)), None);
    }

    #[test]
    fn every_effect_that_changes_a_repository_queues() {
        use crate::effect::{FileOp, LocalOp};
        use trellis_core::repo::{DiffTarget, Hunk};
        let w = PathBuf::from("C:/r");
        let hunk = Hunk { header: String::new(), lines: Vec::new() };
        let changing = [
            Effect::LocalOp { workdir: w.clone(), op: LocalOp::StageAll },
            Effect::RevertHunk { workdir: w.clone(), target: DiffTarget::Commit(git2::Oid::zero(), "a".into()), index: 0, expected: hunk },
            Effect::RestoreFile { workdir: w.clone(), oid: git2::Oid::zero(), path: "a".into(), overwrite: false },
            Effect::WorkFile { workdir: w.clone(), op: FileOp::Create("a".into()) },
            Effect::RunGit {
                cwd: w.clone(),
                cmds: vec![trellis_core::gitcli::args(&["commit"])],
                write_file: None,
                record: None,
                prompts: false,
                expect: None,
            },
            Effect::UndoRedo {
                cwd: w.clone(),
                action: trellis_core::undo::Action::DiscardAll { stash: git2::Oid::zero(), head: git2::Oid::zero() },
                redo: false,
            },
            Effect::InteractiveRebase {
                cwd: w.clone(),
                request: trellis_core::rebase::StartRequest {
                    head: git2::Oid::zero(),
                    upstream: trellis_core::rebase::Upstream::Root,
                    items: vec![],
                },
            },
            Effect::RemoveWorktree(crate::worktree::RemoveRequest {
                repo: w.clone(),
                path: "C:/wt".into(),
                name: "wt".into(),
                force: trellis_core::worktree_ops::RemoveForce::None,
                delete_branch: None,
            }),
            Effect::RenameStash { cwd: w.clone(), index: 0, oid: git2::Oid::zero(), message: "On main: x".into() },
            Effect::SparseCheckout { cwd: w.clone(), command: trellis_core::sparse_ops::SparseCommand::Reapply },
        ];
        for e in &changing {
            assert_eq!(repo_of(e), Some(w.as_path()), "{} must run on its repository's queue", e.name());
        }
    }
}

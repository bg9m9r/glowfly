//! Tests for the per-repository effect queue (runner/queue.rs).

use super::*;
use crate::effect::{Effect, EffectMeta, MonitorJob, MonitorOff, RebaseEffect, ReposEffect, SparseEffect, StashEffect, WorktreeEffect};
use glowfly_core::repo_sync::SyncOp;
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

/// Checks whether a job for `second` waits for a long job for `first` (`same`: one queue
/// for both) or runs meanwhile. Waits generously for the job that should run, so a busy
/// machine can't make two queues look like one.
fn assert_queues(q: &RepoQueues, first: &Path, second: &Path, same: bool) {
    let (gate_tx, gate_rx) = mpsc::channel::<()>();
    let (done_tx, done_rx) = mpsc::channel::<&'static str>();
    let d = done_tx.clone();
    q.submit(
        first,
        Box::new(move || {
            let _ = gate_rx.recv_timeout(Duration::from_secs(30));
            d.send("first").unwrap();
        }),
    )
    .ok()
    .unwrap();
    q.submit(second, Box::new(move || done_tx.send("second").unwrap())).ok().unwrap();
    if same {
        assert!(done_rx.recv_timeout(Duration::from_millis(300)).is_err(), "{} waits for {}", second.display(), first.display());
        gate_tx.send(()).unwrap();
        assert_eq!(done_rx.recv_timeout(Duration::from_secs(30)).unwrap(), "first", "in order");
        assert_eq!(done_rx.recv_timeout(Duration::from_secs(30)).unwrap(), "second");
    } else {
        assert_eq!(done_rx.recv_timeout(Duration::from_secs(30)).unwrap(), "second", "{} doesn't wait", second.display());
        gate_tx.send(()).unwrap();
        assert_eq!(done_rx.recv_timeout(Duration::from_secs(30)).unwrap(), "first");
    }
}

#[test]
fn one_folder_under_two_names_has_one_queue() {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-queue-alias-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let link = dir.with_file_name(format!("glowfly-queue-alias-{}-link", std::process::id()));
    let _ = std::fs::remove_dir(&link).or_else(|_| std::fs::remove_file(&link));
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(&dir, &link).is_ok();
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_dir(&dir, &link).is_ok()
        || std::process::Command::new("cmd").args(["/c", "mklink", "/J"]).arg(&link).arg(&dir).output().is_ok_and(|o| o.status.success());
    assert!(made, "made a link");
    let q = RepoQueues::default();
    assert_queues(&q, &dir, &link, true);
    assert_queues(&q, &dir, &dir.join("."), true);
    let other = dir.with_file_name(format!("glowfly-queue-other-{}", std::process::id()));
    std::fs::create_dir_all(&other).unwrap();
    assert_queues(&q, &dir, &other, false);
    let _ = std::fs::remove_dir(&link).or_else(|_| std::fs::remove_file(&link));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}

#[test]
fn case_twins_share_a_queue_only_where_the_volume_ignores_case() {
    // What the disk would say on a volume that ignores case, and on one that doesn't.
    fn ignores_case(p: &Path) -> Key {
        Key::Spelling(PathBuf::from(p.to_string_lossy().to_lowercase()))
    }
    fn tells_apart(p: &Path) -> Key {
        Key::Spelling(p.to_path_buf())
    }
    let (upper, lower) = (Path::new("/v/App"), Path::new("/v/app"));
    assert_queues(&RepoQueues { router: Mutex::new(None), identify: ignores_case }, upper, lower, true);
    assert_queues(&RepoQueues { router: Mutex::new(None), identify: tells_apart }, upper, lower, false);
}

#[test]
fn which_effects_queue() {
    let w = PathBuf::from("C:/r");
    let git = |cmds: &[&[&str]]| Effect::RunGit {
        cwd: w.clone(),
        cmds: cmds.iter().map(|c| glowfly_core::gitcli::args(c)).collect(),
        write_file: None,
        record: None,
        journal: None,
        prompts: false,
        expect: None,
        stoppable: false,
        on_fail_check: None,
    };
    assert_eq!(Effect::queue(&git(&[&["pull"]])), Some(w.as_path()));
    assert_eq!(Effect::queue(&git(&[&["fetch", "--all"]])), None, "auto-fetch doesn't hold up staging");
    assert_eq!(Effect::queue(&git(&[&["fetch", ".", "a:b"]])), Some(w.as_path()));
    // Fetch or pull across repositories: a pull waits for the repository's other jobs.
    let sync = |op| Effect::Repos(ReposEffect::Sync { generation: 1, index: 0, path: w.clone(), op, trust: None });
    assert_eq!(Effect::queue(&sync(SyncOp::Pull)), Some(w.as_path()));
    assert_eq!(Effect::queue(&sync(SyncOp::Fetch { prune: true })), None);
    // A fetch doesn't queue, but keeps the repository's handles out of the pool (git may repack).
    assert_eq!(Effect::holds(&git(&[&["fetch", "--all"]])), Some(w.as_path()));
    assert_eq!(Effect::holds(&sync(SyncOp::Fetch { prune: true })), Some(w.as_path()));
    assert_eq!(Effect::holds(&git(&[&["pull"]])), Some(w.as_path()));
    // A custom shell command may change anything; reading the commands files changes nothing.
    use glowfly_model::custom_commands::{CustomCommandEffect, ShellRun};
    let shell = Effect::CustomCommand(CustomCommandEffect::RunShell(ShellRun { cwd: w.clone(), script: "make".into() }));
    assert_eq!(Effect::queue(&shell), Some(w.as_path()));
    assert_eq!(Effect::queue(&Effect::CustomCommand(CustomCommandEffect::Load { generation: 1, workdir: w.clone() })), None);
    let op = Effect::LocalOp { workdir: w.clone(), op: crate::effect::LocalOp::StageAll };
    assert_eq!(Effect::queue(&op), Some(w.as_path()));
    assert_eq!(
        Effect::queue(&Effect::LoadStatus {
            workdir: w.clone(),
            generation: 0,
            submodules: false,
            open: None,
            git_status: true,
            paths: None
        }),
        None
    );
    // Saving a patch only reads the repository.
    let what = glowfly_core::patch_ops::PatchSource::Commits { oids: Vec::new(), paths: Vec::new() };
    let export = crate::patch::PatchEffect::Export { workdir: w.clone(), what, dest: w.join("x.patch") };
    assert_eq!(Effect::queue(&Effect::Patch(export)), None);
    // Reading a patch file and copying a commit's patch only read too.
    assert_eq!(Effect::queue(&Effect::Patch(crate::patch::PatchEffect::Inspect { workdir: w.clone(), path: w.join("x.patch") })), None);
    assert_eq!(Effect::queue(&Effect::Patch(crate::patch::PatchEffect::Copy { workdir: w.clone(), oids: Vec::new() })), None);
    assert_eq!(Effect::queue(&Effect::Tool(glowfly_model::tools::ToolEffect::PickProgram(glowfly_model::tools::ToolKind::Editor))), None);
    // A conflict check writes only unreferenced objects.
    let request = glowfly_model::conflict_check::CheckRequest {
        branch: "feature".into(),
        ours: git2::Oid::zero(),
        target: glowfly_core::conflict_check::MergeTarget {
            name: "main".into(),
            oid: git2::Oid::zero(),
            kind: glowfly_core::conflict_check::TargetKind::DefaultBranch,
        },
    };
    let job = glowfly_model::conflict_check::CheckJob { workdir: w.clone(), request, manual: false, generation: 0 };
    assert_eq!(Effect::queue(&Effect::CheckConflicts(job)), None);
    // Switching profiles writes the global git config only.
    let identity = glowfly_core::identity::Identity::default();
    let apply = crate::profile::ProfileEffect::Apply { cwd: w.clone(), has_repo: true, id: glowfly_core::settings::ProfileId(1), identity };
    assert_eq!(Effect::queue(&Effect::Profile(apply)), None);
    // Folder rules and a repository's identity never wait behind its jobs.
    let id = glowfly_core::settings::ProfileId(1);
    let plan = glowfly_core::profile_includes::IncludePlan { files: Vec::new(), entries: Vec::new() };
    let (workdir, git_dir) = (w.clone(), w.join(".git"));
    for e in [
        crate::profile::ProfileEffect::SyncIncludes { cwd: w.clone(), plan, place: true },
        crate::profile::ProfileEffect::ReadIdentity { workdir: workdir.clone(), git_dir: git_dir.clone(), rules: Vec::new() },
        crate::profile::ProfileEffect::ClearLocalIdentity { workdir, git_dir, rules: Vec::new() },
        crate::profile::ProfileEffect::CheckFolder { id, path: w.clone(), repository: true },
    ] {
        assert_eq!(Effect::queue(&Effect::Profile(e)), None);
    }
    // So does the welcome's identity step.
    let write = glowfly_model::onboarding::OnboardingEffect::WriteIdentity { cwd: w.clone(), identity: Default::default() };
    assert_eq!(Effect::queue(&Effect::Onboarding(write)), None);
    // Saving a typed sign-in talks to git's credential helper only.
    let secret = glowfly_core::askpass::Secret::new("hunter2");
    let credential = glowfly_core::git_credential::HttpsCredential::from_prompt("https://github.com", "ada", secret).unwrap();
    let remember = crate::effect::CredentialEffect::Remember { cwd: w.clone(), credential };
    assert_eq!(Effect::queue(&Effect::Credential(remember)), None);
    // Fork lists and forking call the hosting service only; the remote is added by a job.
    use crate::remote::{ForkRequest, RemoteEffect};
    let target = glowfly_core::forge::parse_remote("https://github.com/team/app", &[]).unwrap();
    let req = || ForkRequest { workdir: w.clone(), target: target.clone(), auth: None, connected: false };
    let remote =
        [RemoteEffect::ListForks(req()), RemoteEffect::FindMyFork(req()), RemoteEffect::CreateFork { req: req(), name: "ada".into() }];
    let names: Vec<&str> = remote.iter().map(RemoteEffect::name).collect();
    assert_eq!(names, ["list-forks", "find-my-fork", "create-fork"]);
    for e in remote {
        let e = Effect::Remote(e);
        assert_eq!((Effect::queue(&e), e.scope()), (None, None), "{}", e.name());
    }
}

#[test]
fn every_effect_that_changes_a_repository_queues() {
    use crate::effect::{FileOp, LocalOp};
    use glowfly_core::repo::{DiffTarget, RevertPart, RevertRequest};
    let w = PathBuf::from("C:/r");
    let changing = [
        Effect::LocalOp { workdir: w.clone(), op: LocalOp::StageAll },
        Effect::Revert {
            workdir: w.clone(),
            req: RevertRequest {
                target: DiffTarget::Commit(git2::Oid::zero(), "a".into()),
                part: RevertPart::File,
                dest: None,
                ignore_whitespace: false,
            },
        },
        Effect::RestoreFile { workdir: w.clone(), oid: git2::Oid::zero(), path: "a".into(), overwrite: false },
        Effect::WorkFile { workdir: w.clone(), op: FileOp::Create("a".into()) },
        Effect::RunGit {
            cwd: w.clone(),
            cmds: vec![glowfly_core::gitcli::args(&["commit"])],
            write_file: None,
            record: None,
            journal: None,
            prompts: false,
            expect: None,
            stoppable: false,
            on_fail_check: None,
        },
        Effect::UndoRedo {
            cwd: w.clone(),
            action: glowfly_core::undo::Action::DiscardAll { stash: git2::Oid::zero(), head: git2::Oid::zero() },
            redo: false,
            opts: Default::default(),
        },
        Effect::Rebase(RebaseEffect::Start {
            cwd: w.clone(),
            request: glowfly_core::rebase::StartRequest {
                head: git2::Oid::zero(),
                upstream: glowfly_core::rebase::Upstream::Root,
                items: vec![],
                autostash: false,
            },
            detail: String::new(),
        }),
        Effect::Rebase(RebaseEffect::Resume { cwd: w.clone(), how: glowfly_core::rebase::Resume::Skip }),
        Effect::Rebase(RebaseEffect::EditTodo {
            cwd: w.clone(),
            request: glowfly_core::rebase::EditTodoRequest {
                steps: vec![],
                locked_text: String::new(),
                todo: String::new(),
                first_may_combine: false,
            },
        }),
        Effect::Worktree(WorktreeEffect::Remove(crate::worktree::RemoveRequest {
            repo: w.clone(),
            path: "C:/wt".into(),
            name: "wt".into(),
            force: glowfly_core::worktree_ops::RemoveForce::None,
            delete_branch: None,
            delete_remote: None,
            stop_monitor: false,
        })),
        Effect::Worktree(WorktreeEffect::Move(crate::worktree::MoveRequest {
            repo: w.clone(),
            from: "C:/wt".into(),
            to: "C:/moved/wt".into(),
            name: "wt".into(),
            folder: None,
            stop_monitor: false,
        })),
        Effect::Stash(StashEffect::Rename { cwd: w.clone(), index: 0, oid: git2::Oid::zero(), message: "On main: x".into() }),
        Effect::Sparse(SparseEffect::Checkout { cwd: w.clone(), command: glowfly_core::sparse_ops::SparseCommand::Reapply }),
        Effect::TurnOnMonitor(MonitorJob { cwd: w.clone(), then: vec![] }),
        Effect::TurnOffMonitor(MonitorOff { cwd: w.clone() }),
    ];
    for e in &changing {
        assert_eq!(Effect::queue(e), Some(w.as_path()), "{} must run on its repository's queue", e.name());
    }
}

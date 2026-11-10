//! Tests for the effect areas' metadata (effect/meta.rs): the effects that moved into areas keep
//! the names, scopes, queues, timing and sandbox rules they had as flat variants, and the pool
//! holds and stop keys are unchanged.

use crate::effect::partial::{ContentRequest, PartialEffect, StopKey};
use crate::effect::{
    CloneEffect, CommitEffect, ConflictEffect, ConflictRead, DiffEffect, Effect, FileOp, FilesEffect, IndexRequest, JobEffect, LfsEffect,
    LoadEffect, LoadKind, MonitorJob, MonitorOff, ReposEffect, SandboxRule, SelectionEffect, SparseEffect, StackRequest, UndoOpts,
    WorktreeEffect,
};
use crate::worktree::{CountRequest, MoveRequest, NewWorktree, RemoveRequest};
use glowfly_core::repo_sync::SyncOp;
use std::path::{Path, PathBuf};

fn folder(name: &str) -> PathBuf {
    glowfly_core::paths::temp_dir().join(format!("glowfly-meta-{name}"))
}

/// An effect's (name, scope, queue, timed).
type Meta<'a> = (&'static str, Option<&'a Path>, Option<&'a Path>, bool);

fn meta(e: &Effect) -> Meta<'_> {
    (e.name(), e.scope().map(PathBuf::as_path), e.queue(), e.timed())
}

fn git(cwd: &Path, cmds: &[&[&str]], stoppable: bool) -> Effect {
    Effect::Job(JobEffect::RunGit {
        cwd: cwd.to_owned(),
        cmds: cmds.iter().map(|c| glowfly_core::gitcli::args(c)).collect(),
        write_file: None,
        record: None,
        journal: None,
        prompts: false,
        expect: None,
        stoppable,
        on_fail_check: None,
        auto_stash: None,
    })
}

#[test]
fn moved_effects_keep_their_metadata() {
    let w = folder("work");
    let (cwd, repo, other) = (folder("cwd"), folder("repo"), folder("other"));
    let oid = git2::Oid::zero();
    let scoped = Some(w.as_path());
    let cases: Vec<(Effect, Meta)> = vec![
        (Effect::Lfs(LfsEffect::Detect), ("lfs-detect", None, None, true)),
        (Effect::Lfs(LfsEffect::Load { workdir: w.clone(), generation: 1 }), ("lfs-status", scoped, None, true)),
        (Effect::Sparse(SparseEffect::Load { workdir: w.clone(), generation: 1, run_git: true }), ("sparse-status", scoped, None, true)),
        (
            Effect::Sparse(SparseEffect::Checkout { cwd: cwd.clone(), command: glowfly_core::sparse_ops::SparseCommand::Reapply }),
            ("sparse-checkout", None, Some(cwd.as_path()), true),
        ),
        (Effect::TurnOnMonitor(MonitorJob { cwd: cwd.clone(), then: Vec::new() }), ("turn-on-monitor", None, Some(cwd.as_path()), true)),
        (Effect::TurnOffMonitor(MonitorOff { cwd: cwd.clone() }), ("turn-off-monitor", None, Some(cwd.as_path()), true)),
        (
            Effect::Conflict(ConflictEffect::Load { workdir: w.clone(), path: "a.txt".into(), generation: 1 }),
            ("load-conflict", scoped, None, true),
        ),
        (
            Effect::Conflict(ConflictEffect::ReadFiles {
                workdir: w.clone(),
                paths: vec!["a.txt".into()],
                purpose: ConflictRead::BeforeMarkResolved,
            }),
            ("conflict-files", scoped, None, true),
        ),
        (Effect::Selection(SelectionEffect::Compare { workdir: w.clone(), from: oid, to: oid }), ("load-compare", scoped, None, true)),
        (
            Effect::Selection(SelectionEffect::WorkdirCompare { workdir: w.clone(), oid, generation: 1, swapped: false }),
            ("load-workdir-compare", scoped, None, true),
        ),
        (
            Effect::Selection(SelectionEffect::Stacked(StackRequest { workdir: w.clone(), oids: vec![oid] })),
            ("load-stacked", scoped, None, true),
        ),
        (
            Effect::Clone(CloneEffect::CheckParent { parent: "parent".into(), delay: std::time::Duration::ZERO }),
            ("check-clone-parent", None, None, true),
        ),
        (
            Effect::Worktree(WorktreeEffect::CheckFolder(NewWorktree {
                repo: repo.clone(),
                base: glowfly_core::worktree_ops::WorktreeBase::Commit(oid),
                folder: other.clone(),
                branch: None,
                open: false,
            })),
            ("check-worktree-folder", None, None, true),
        ),
        (
            Effect::Worktree(WorktreeEffect::Remove(RemoveRequest {
                repo: repo.clone(),
                path: other.clone(),
                name: "other".into(),
                force: glowfly_core::worktree_ops::RemoveForce::None,
                delete_branch: None,
                delete_remote: None,
                stop_monitor: false,
            })),
            ("remove-worktree", None, Some(repo.as_path()), true),
        ),
        (
            Effect::Worktree(WorktreeEffect::CountChanges(CountRequest { workdir: w.clone(), refs_key: 1, targets: Vec::new() })),
            ("count-worktree-changes", scoped, None, true),
        ),
        (
            Effect::Worktree(WorktreeEffect::Move(MoveRequest {
                repo: repo.clone(),
                from: other.clone(),
                to: folder("moved"),
                name: "other".into(),
                folder: None,
                stop_monitor: false,
            })),
            ("move-worktree", None, Some(repo.as_path()), true),
        ),
        (
            Effect::Files(FilesEffect::Restore { workdir: w.clone(), oid, path: "a.txt".into(), overwrite: false }),
            ("restore-file", scoped, scoped, true),
        ),
        (Effect::Files(FilesEffect::Work { workdir: w.clone(), op: FileOp::Create("a.txt".into()) }), ("work-file", scoped, scoped, true)),
        (Effect::Files(FilesEffect::LoadTracked { workdir: w.clone(), generation: 1 }), ("tracked-paths", scoped, None, true)),
        (Effect::Commit(CommitEffect::LoadDetail { workdir: w.clone(), oid }), ("load-detail", scoped, None, true)),
        (
            Effect::Commit(CommitEffect::VerifySignature {
                workdir: w.clone(),
                oid,
                format: glowfly_core::signing::SigningFormat::OpenPgp,
                generation: 1,
            }),
            ("verify-signature", scoped, None, true),
        ),
    ];
    for (effect, expected) in &cases {
        assert_eq!(meta(effect), *expected, "{effect:?}");
        assert_eq!(effect.holds(), expected.2, "{} holds what it queues on", expected.0);
        assert_eq!(effect.stop_key(), None, "{} can't be stopped", expected.0);
        if matches!(effect, Effect::Files(_) | Effect::Commit(_)) {
            assert_eq!(effect.sandbox(), SandboxRule::Run, "{} runs in the whole-app tests", expected.0);
        }
    }
}

/// The Load, Diff and Job areas (#373): what main gave the flat `LoadRepo`, `LoadStatus`,
/// `LoadIndexStatus`, `LoadDiff`, `HighlightDiff`, `LoadDiffGaps`, `LoadBinarySides`, `RunGit` and
/// `UndoRedo`.
#[test]
fn load_diff_and_job_effects_keep_their_metadata() {
    let w = folder("reads");
    let cwd = folder("job");
    let oid = git2::Oid::zero();
    let scoped = Some(w.as_path());
    let target = || glowfly_core::repo::DiffTarget::Unstaged("a.txt".into());
    let diff = || {
        let d =
            glowfly_core::repo::FileDiff { binary: false, hunks: Vec::new(), highlight: None, intraline: Default::default(), whole: None };
        std::sync::Arc::new(d)
    };
    let memo = std::sync::Arc::new(glowfly_core::repo::IndexMemo::of(&git2::Index::new().unwrap()).unwrap());
    let read = IndexRequest { old: memo, partners: Vec::new(), paths: Vec::new() };
    let action = glowfly_core::undo::Action::Commit { branch: None, from: oid, to: oid, message: String::new(), amend: false };
    let cases: Vec<(Effect, Meta)> = vec![
        (
            Effect::Load(LoadEffect::Repo {
                path: w.clone(),
                opts: Default::default(),
                generation: 1,
                kind: LoadKind::Refresh,
                known: None,
                prior: None,
            }),
            ("load-repo", None, None, true),
        ),
        (
            Effect::Load(LoadEffect::Status {
                workdir: w.clone(),
                generation: 1,
                submodules: false,
                open: None,
                git_status: true,
                paths: None,
            }),
            ("load-status", scoped, None, true),
        ),
        (
            Effect::Load(LoadEffect::IndexStatus {
                workdir: w.clone(),
                generation: 1,
                submodules: false,
                open: None,
                git_status: true,
                read,
            }),
            ("load-index-status", scoped, None, true),
        ),
        (
            Effect::Diff(DiffEffect::Load {
                workdir: w.clone(),
                target: target(),
                reload: false,
                generation: 1,
                previous: None,
                ignore_whitespace: false,
                whole_file: false,
                listed: None,
            }),
            ("load-diff", scoped, None, true),
        ),
        (
            Effect::Diff(DiffEffect::Highlight { workdir: w.clone(), target: target(), generation: 1, diff: diff() }),
            ("highlight", scoped, None, true),
        ),
        (
            Effect::Diff(DiffEffect::Gaps { workdir: w.clone(), target: target(), generation: 1, diff: diff() }),
            ("load-diff-gaps", scoped, None, true),
        ),
        (
            Effect::Diff(DiffEffect::BinarySides { workdir: w.clone(), target: target(), generation: 1 }),
            ("binary-sides", scoped, None, true),
        ),
        (git(&cwd, &[&["commit"]], false), ("git", None, Some(cwd.as_path()), true)),
        (
            Effect::Job(JobEffect::UndoRedo { cwd: cwd.clone(), action, redo: false, opts: UndoOpts::default() }),
            ("undo-redo", None, Some(cwd.as_path()), true),
        ),
    ];
    for (effect, expected) in &cases {
        assert_eq!(meta(effect), *expected, "{effect:?}");
        assert_eq!(effect.holds(), expected.2, "{} holds what it queues on", expected.0);
        assert_eq!(effect.stop_key(), None, "{} can't be stopped", expected.0);
        assert_eq!(effect.sandbox(), SandboxRule::Run, "{} runs in the whole-app tests", expected.0);
    }
}

/// The Files, History, Save and Session areas (#373): what main gave the flat `LocalOp`,
/// `Revert`, `LoadFileHistory`, `SearchPaths`, `Stats`, `SaveSettings`, `SaveRecent`, `SaveTabs`,
/// `CheckRepoFolders` and `ReadTabGroups`.
#[test]
fn files_history_save_and_session_effects_keep_their_metadata() {
    use crate::effect::{HistoryEffect, LocalOp, SaveEffect, SessionEffect};
    use glowfly_core::repo::{DiffTarget, HistoryStart, RevertPart, RevertRequest};
    let w = folder("moved-3");
    let oid = git2::Oid::zero();
    let scoped = Some(w.as_path());
    let req =
        RevertRequest { target: DiffTarget::Commit(oid, "a.txt".into()), part: RevertPart::File, dest: None, ignore_whitespace: false };
    let cases: Vec<(Effect, Meta, SandboxRule)> = vec![
        (
            Effect::Files(FilesEffect::LocalOp { workdir: w.clone(), op: LocalOp::StageAll }),
            ("local-op", scoped, scoped, true),
            SandboxRule::Run,
        ),
        (Effect::Files(FilesEffect::Revert { workdir: w.clone(), req }), ("revert", scoped, scoped, true), SandboxRule::Run),
        (
            Effect::History(HistoryEffect::FileHistory {
                workdir: w.clone(),
                path: "a.txt".into(),
                start: HistoryStart::Head,
                limit: 10,
                folder: false,
                generation: 1,
            }),
            ("file-history", scoped, None, true),
            SandboxRule::Run,
        ),
        (
            Effect::History(HistoryEffect::SearchPaths { workdir: w.clone(), request: 1, terms: Vec::new(), oids: vec![oid] }),
            ("search-paths", scoped, None, true),
            SandboxRule::Run,
        ),
        (Effect::History(HistoryEffect::Stats { workdir: w.clone(), oids: vec![oid] }), ("stats", scoped, None, true), SandboxRule::Run),
        (Effect::Save(SaveEffect::Settings(Box::default())), ("save-settings", None, None, true), SandboxRule::Save),
        (Effect::Save(SaveEffect::Recent(vec![w.clone()])), ("save-recent", None, None, true), SandboxRule::Save),
        (Effect::Save(SaveEffect::ClearRecent), ("clear-recent", None, None, true), SandboxRule::Save),
        (
            Effect::Save(SaveEffect::Tabs { paths: vec![w.clone()], labels: vec![None], active: 0 }),
            ("save-tabs", None, None, true),
            SandboxRule::Save,
        ),
        (Effect::Session(SessionEffect::CheckRepoFolders(vec![w.clone()])), ("check-repo-folders", None, None, true), SandboxRule::Run),
        (Effect::Session(SessionEffect::ReadTabGroups(vec![w.clone()])), ("read-tab-groups", None, None, true), SandboxRule::Run),
    ];
    for (effect, expected, sandbox) in &cases {
        assert_eq!(meta(effect), *expected, "{effect:?}");
        assert_eq!(effect.holds(), expected.2, "{} holds what it queues on", expected.0);
        assert_eq!(effect.stop_key(), None, "{} can't be stopped", expected.0);
        assert_eq!(effect.sandbox(), *sandbox, "{}'s whole-app tests rule", expected.0);
    }
}

/// The Network and Control areas (#373): what main gave the flat `Forge`, `FetchAvatars`,
/// `Watch`, `SweepWatch`, `AnswerPrompt` and `StopGit`, with the pool hold and stop key too.
#[test]
fn network_and_control_effects_keep_their_metadata() {
    use crate::effect::{ControlEffect, ForgeCall, NetworkEffect, WatchTarget};
    use glowfly_core::askpass::Reply;
    use glowfly_core::forge::{Provider, Target};
    let w = folder("moved-4");
    let target =
        Target { provider: Provider::GitHub, host: "github.com".into(), owner: "o".into(), project: String::new(), repo: "r".into() };
    let call = ForgeCall::Refresh { need_default: false, with_issues: false };
    let forge = NetworkEffect::Forge { target, cwd: w.clone(), auth: None, interactive: false, connected: false, call };
    let watch = WatchTarget { workdir: w.clone(), git_dir: w.join(".git") };
    let cases: Vec<(Effect, Meta, SandboxRule)> = vec![
        (Effect::Network(forge), ("forge", Some(w.as_path()), None, true), SandboxRule::Drop),
        (Effect::Network(NetworkEffect::Avatars(vec!["ada@example.invalid".into()])), ("avatars", None, None, true), SandboxRule::Drop),
        (Effect::Control(ControlEffect::Watch(Some(watch))), ("watch", None, None, false), SandboxRule::Run),
        (Effect::Control(ControlEffect::Watch(None)), ("watch", None, None, false), SandboxRule::Run),
        (Effect::Control(ControlEffect::SweepWatch { workdir: w.clone() }), ("sweep-watch", None, None, false), SandboxRule::Run),
        (
            Effect::Control(ControlEffect::AnswerPrompt { id: 1, reply: Reply::Cancel }),
            ("answer-prompt", None, None, false),
            SandboxRule::Run,
        ),
        (Effect::Control(ControlEffect::StopGit(StopKey::Job)), ("stop-git", None, None, false), SandboxRule::Run),
    ];
    for (effect, expected, sandbox) in &cases {
        assert_eq!(meta(effect), *expected, "{effect:?}");
        assert_eq!(effect.holds(), None, "{} holds no repository", expected.0);
        assert_eq!(effect.stop_key(), None, "{} can't be stopped", expected.0);
        assert_eq!(effect.sandbox(), *sandbox, "{}'s whole-app tests rule", expected.0);
    }
}

/// `RunGit`'s queue, pool hold and stop key as main had them: a job of only fetches and pushes
/// doesn't queue but holds the pool, one that names `.` or runs anything else queues, and only a
/// `stoppable` job has a stop.
#[test]
fn a_git_job_queues_holds_and_stops_as_before() {
    let w = folder("git-job");
    let at = Some(w.clone());
    let rules = |e: Effect| {
        (e.name(), e.scope().is_none(), e.queue().map(Path::to_path_buf), e.holds().map(Path::to_path_buf), e.stop_key(), e.sandbox())
    };
    let run = SandboxRule::Run;
    assert_eq!(rules(git(&w, &[&["fetch", "origin"], &["push", "origin", "main"]], false)), ("git", true, None, at.clone(), None, run));
    assert_eq!(rules(git(&w, &[&["fetch", "origin"]], true)), ("git", true, None, at.clone(), Some(StopKey::Job), run));
    assert_eq!(
        rules(git(&w, &[&["fetch", "."]], false)),
        ("git", true, at.clone(), at.clone(), None, run),
        "a fetch into this repository changes it"
    );
    assert_eq!(rules(git(&w, &[&["fetch", "origin"], &["merge"]], true)), ("git", true, at.clone(), at.clone(), Some(StopKey::Job), run));
    assert_eq!(rules(git(&w, &[&["commit"]], false)), ("git", true, at.clone(), at, None, run));
}

#[test]
fn holds_and_stop_keys_are_unchanged() {
    let w = folder("pool");
    let fetch = git(&w, &[&["fetch", "--all"]], false);
    assert_eq!((fetch.queue(), fetch.holds()), (None, Some(w.as_path())), "a fetch doesn't queue but holds the pool");
    assert_eq!(fetch.stop_key(), None);
    let stoppable = git(&w, &[&["fetch", "--all"]], true);
    assert_eq!(stoppable.stop_key(), Some(StopKey::Job));
    let content = Effect::Partial(PartialEffect::FetchContent(ContentRequest { workdir: w.clone(), id: 7, cmds: Vec::new() }));
    assert_eq!(content.stop_key(), Some(StopKey::Content(7)));
    let sync = |op| Effect::Repos(ReposEffect::Sync { generation: 1, index: 0, path: w.clone(), op, trust: None });
    let fetch_all = sync(SyncOp::Fetch { prune: false });
    assert_eq!((fetch_all.queue(), fetch_all.holds()), (None, Some(w.as_path())));
    let pull = sync(SyncOp::Pull);
    assert_eq!((pull.queue(), pull.holds()), (Some(w.as_path()), Some(w.as_path())));
}

#[test]
fn prefs_effects_are_timed_unless_they_wait_on_the_passphrase() {
    use crate::effect::PrefsEffect;
    use glowfly_core::signing::SigningFormat;
    let cwd = folder("prefs");
    let timed = |e: PrefsEffect| Effect::Prefs(e).timed();
    assert!(timed(PrefsEffect::ReadGitConfig { cwd: cwd.clone(), has_repo: true }));
    assert!(timed(PrefsEffect::SetGitConfig { cwd, global: false, key: "user.name".into(), value: None }));
    assert!(timed(PrefsEffect::ListSigningKeys { format: SigningFormat::OpenPgp, program: "gpg".into() }));
    assert!(timed(PrefsEffect::ExportPublicKey { format: SigningFormat::OpenPgp, program: "gpg".into(), key: "k".into() }));
    let generate = PrefsEffect::GenerateGpgKey { program: "gpg".into(), name: "n".into(), email: "e".into(), passphrase: String::new() };
    assert!(!timed(generate), "waits on the passphrase dialog");
    assert!(!timed(PrefsEffect::ExtendGpgKey { program: "gpg".into(), key: "k".into() }), "waits on the passphrase prompt");
}

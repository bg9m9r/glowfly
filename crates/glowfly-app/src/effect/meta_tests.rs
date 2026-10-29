//! Tests for the effect areas' metadata (effect/meta.rs): the effects that moved into areas keep
//! the names, scopes, queues and timing they had as flat variants, and the pool holds and stop
//! keys are unchanged.

use crate::effect::partial::{ContentRequest, PartialEffect, StopKey};
use crate::effect::{
    CloneEffect, ConflictEffect, ConflictRead, Effect, LfsEffect, MonitorJob, MonitorOff, ReposEffect, SelectionEffect, SparseEffect,
    StackRequest, WorktreeEffect,
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
    Effect::RunGit {
        cwd: cwd.to_owned(),
        cmds: cmds.iter().map(|c| glowfly_core::gitcli::args(c)).collect(),
        write_file: None,
        record: None,
        journal: None,
        prompts: false,
        expect: None,
        stoppable,
        on_fail_check: None,
    }
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
    ];
    for (effect, expected) in &cases {
        assert_eq!(meta(effect), *expected, "{effect:?}");
        assert_eq!(effect.holds(), expected.2, "{} holds what it queues on", expected.0);
        assert_eq!(effect.stop_key(), None, "{} can't be stopped", expected.0);
    }
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
}

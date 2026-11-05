//! An auto-stash job's record is written by the runner as soon as the stash exists, and `Made`
//! is sent before the job's own result; nothing is written when nothing was stashed or the job
//! isn't an auto-stash.

use crate::testing::execute_all;
use glowfly_core::gitcli::args;
use glowfly_core::pick_ops::Picking;
use glowfly_core::stash_ops::{self, AutoStashFor};
use glowfly_core::testkit::git;
use glowfly_model::effect::{Effect, JobEffect, PickJob};
use glowfly_model::msg::{AutoStashMsg, JobMsg, Msg};
use glowfly_model::state::auto_stash::AutoStashJob;
use glowfly_model::testing::scratch_repo;
use std::path::{Path, PathBuf};

fn op() -> AutoStashFor {
    AutoStashFor::Checkout { target: "nosuch".to_owned() }
}

/// The auto-stash of `op` for `dir` as the model builds it: the stashes there are now in `before`.
fn job(dir: &Path, op: AutoStashFor) -> AutoStashJob {
    let before = stash_ops::list(dir).unwrap().iter().map(|s| s.oid).collect();
    AutoStashJob { message: op.message(), before, op }
}

fn run_git(dir: &Path, cmds: Vec<Vec<String>>, auto_stash: Option<AutoStashJob>) -> Vec<Msg> {
    execute_all(Effect::Job(JobEffect::RunGit {
        cwd: dir.to_path_buf(),
        cmds,
        write_file: None,
        record: None,
        journal: None,
        prompts: false,
        expect: None,
        stoppable: false,
        on_fail_check: None,
        auto_stash,
    }))
}

fn file(dir: &Path) -> PathBuf {
    stash_ops::saved_path(dir).unwrap()
}

/// A checkout that fails after the stash: the record names stash@{0} and `Made` comes first.
#[test]
fn the_record_is_written_once_the_stash_is_made_and_made_comes_before_the_result() {
    let dir = scratch_repo("auto-stash-made-run-git");
    git(&dir, &["stash", "push", "-m", "older"]);
    std::fs::write(dir.join("a.txt"), "mine\n").unwrap();
    let a = job(&dir, op());
    let cmds = vec![op().stash_command(), args(&["checkout", "nosuch"])];
    let msgs = run_git(&dir, cmds, Some(a));
    let top = stash_ops::list(&dir).unwrap()[0].oid;
    let saved = stash_ops::read_saved(&dir);
    assert_eq!(saved.len(), 1, "{saved:?}");
    assert_eq!((saved[0].oid(), &saved[0].op, saved[0].message.as_str()), (Some(top), &op(), op().message().as_str()));
    match &msgs[..] {
        [Msg::AutoStash(AutoStashMsg::Made { workdir, oid }), Msg::Job(JobMsg::JobDone { ok: false, .. })] => {
            assert_eq!((workdir.as_path(), *oid), (dir.as_path(), top));
        }
        other => panic!("Made, then the failed job; got {} messages", other.len()),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_stashed_or_no_auto_stash_writes_nothing() {
    let dir = scratch_repo("auto-stash-made-nothing");
    git(&dir, &["checkout", "--", "a.txt"]);
    let cmds = || vec![op().stash_command(), args(&["status", "--short"])];
    // "No local changes to save": the push succeeds and makes no entry.
    let a = job(&dir, op());
    let msgs = run_git(&dir, cmds(), Some(a));
    assert!(matches!(&msgs[..], [Msg::Job(JobMsg::JobDone { ok: true, .. })]), "{} messages", msgs.len());
    assert!(!file(&dir).exists());
    // A plain job (the Stash dialog's push) never writes a record.
    std::fs::write(dir.join("a.txt"), "mine\n").unwrap();
    let msgs = run_git(&dir, cmds(), None);
    assert!(matches!(&msgs[..], [Msg::Job(JobMsg::JobDone { ok: true, .. })]), "{} messages", msgs.len());
    assert_eq!(stash_ops::list(&dir).unwrap().len(), 1, "the push did stash");
    assert!(!file(&dir).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A cherry-pick that stops on conflicts after its auto-stash push: the record is there, and
/// `Made` comes before `PickDone`.
#[test]
fn a_pick_job_writes_the_record_before_it_picks() {
    let dir = scratch_repo("auto-stash-made-pick");
    git(&dir, &["checkout", "--", "a.txt"]);
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "b"]);
    git(&dir, &["checkout", "-q", "-b", "side"]);
    std::fs::write(dir.join("a.txt"), "side\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "side"]);
    let theirs = git2::Repository::open(&dir).unwrap().head().unwrap().target().unwrap();
    git(&dir, &["checkout", "-q", "main"]);
    std::fs::write(dir.join("a.txt"), "main\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "main"]);
    std::fs::write(dir.join("b.txt"), "mine\n").unwrap();
    let op = AutoStashFor::CherryPick { commits: theirs.to_string()[..7].to_owned(), onto: "main".to_owned() };
    let a = job(&dir, op.clone());
    let msgs = execute_all(Effect::Pick(PickJob {
        cwd: dir.clone(),
        cmds: vec![op.stash_command(), args(&["cherry-pick", &theirs.to_string()])],
        record: None,
        journal: None,
        prompts: false,
        picking: Picking::CherryPick,
        oids: vec![theirs],
        can_skip: true,
        auto_stash: Some(a),
    }));
    let top = stash_ops::list(&dir).unwrap()[0].oid;
    assert_eq!(stash_ops::read_saved(&dir).iter().map(|s| s.oid()).collect::<Vec<_>>(), vec![Some(top)]);
    match &msgs[..] {
        [Msg::AutoStash(AutoStashMsg::Made { oid, .. }), Msg::Job(JobMsg::PickDone { ok: false, .. })] => assert_eq!(*oid, top),
        other => panic!("Made, then the stopped pick; got {} messages", other.len()),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

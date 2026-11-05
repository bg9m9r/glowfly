//! The auto-stash's record is saved as soon as the stash exists (`AutoStashMsg::Made`): a
//! session that ends, or a crash, anywhere between the stash and the end of its put-back still
//! offers it at the next open; every end of the put-back removes it; and the running job's own
//! record is kept by every rewrite but never offered while it runs.

use super::auto_stash_flow_tests::mine_is_back;
use super::auto_stash_saved_tests::{file, offered, open};
use super::auto_stash_tests::up;
use super::cherry_pick_tests::feature_repo;
use super::tests::{drive, finish, git, skipped, state};
use git2::Oid;
use glowfly_core::stash_ops::{self, AutoStashFor, PutBack, SavedAutoStash};
use glowfly_model::effect::{Effect, JobEffect, StashEffect};
use glowfly_model::msg::{AutoStashMsg, JobMsg, Msg, RepoMsg};
use glowfly_model::state::AppState;
use glowfly_model::state::auto_stash::{AutoStashJob, Making, PendingStash, Phase, SavedOffer};
use std::path::{Path, PathBuf};
use std::time::Instant;

const MESSAGE: &str = "Auto-stash before checking out feature";

/// A repository with a.txt edited, its "Stash and check out" feature answered: the state and the
/// effects that answer raised (the job's RunGit among them).
fn stash_and_checkout(name: &str) -> (AppState, PathBuf, Vec<Effect>) {
    let (mut st, dir, _) = feature_repo(name, &[("feature work", "feature\n")], None);
    std::fs::write(dir.join("a.txt"), "one\nmine\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    assert!(up(&mut st, Msg::git("Checkout", &["checkout", "feature", "--"])).is_empty(), "the prompt");
    let fx = up(&mut st, Msg::AutoStash(AutoStashMsg::Stash));
    (st, dir, fx)
}

/// Runs the job's RunGit from `fx` (and nothing else), returning what the runner sent and its result.
fn run_the_job(fx: Vec<Effect>) -> Vec<Msg> {
    let job = fx.into_iter().find(|e| matches!(e, Effect::Job(JobEffect::RunGit { .. }))).expect("the job");
    glowfly_runner::testing::execute_all(job)
}

fn top_stash(dir: &Path) -> Oid {
    stash_ops::list(dir).unwrap()[0].oid
}

/// The records of every SaveAutoStashes in `fx`, in order.
fn saves(fx: &[Effect]) -> Vec<Vec<Option<Oid>>> {
    let records = |s: &[SavedAutoStash]| s.iter().map(|r| r.oid()).collect();
    fx.iter()
        .filter_map(|e| if let Effect::Stash(StashEffect::SaveAutoStashes { saved, .. }) = e { Some(records(saved)) } else { None })
        .collect()
}

/// The next open offers the stash of `dir` by its message.
fn offered_at_open(dir: &Path) -> AppState {
    let st = open(dir);
    let body = offered(&st).expect("the offer");
    assert!(body.contains(MESSAGE), "{body}");
    st
}

#[test]
fn a_session_ended_mid_job_still_offers_the_stash() {
    let (mut st, dir, fx) = stash_and_checkout("asm-mid-job");
    let mut msgs = run_the_job(fx);
    let stash = top_stash(&dir);
    assert!(matches!(msgs.last(), Some(Msg::Job(JobMsg::JobDone { ok: true, .. }))), "the job's own result comes last");
    let made = msgs.remove(0);
    assert!(matches!(&made, Msg::AutoStash(AutoStashMsg::Made { oid, .. }) if *oid == stash), "Made comes first");
    let fx = up(&mut st, made);
    assert_eq!(st.auto_stash.making.as_ref().and_then(|m| m.made), Some(stash));
    assert_eq!(saves(&fx), vec![vec![Some(stash)]], "the rewrite keeps the running job's record");
    // The job's end never arrives: the session is over.
    drop(st);
    assert_eq!(stash_ops::read_saved(&dir).iter().map(|s| s.oid()).collect::<Vec<_>>(), vec![Some(stash)]);
    let mut st = offered_at_open(&dir);
    drive(&mut st, Msg::ConfirmAccepted);
    mine_is_back(&st, &dir);
    assert!(!file(&dir).exists(), "the record is removed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_crash_before_glowfly_heard_of_the_stash_still_offers_it() {
    let (st, dir, fx) = stash_and_checkout("asm-crash");
    // None of the job's messages reach the model: the runner's own write is all there is.
    let _ = run_the_job(fx);
    drop(st);
    let mut st = offered_at_open(&dir);
    drive(&mut st, Msg::ConfirmAlt);
    assert!(!file(&dir).exists(), "Leave in stash list removes the record");
    assert_eq!(stash_ops::list(&dir).unwrap().len(), 1, "and keeps the stash");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `finish`, except that putting an auto-stash back never runs.
fn finish_but_put_back(st: &mut AppState, fx: Vec<Effect>) {
    let (mut queue, mut fx) = (Vec::new(), fx);
    loop {
        for effect in fx {
            if !skipped(&effect) && !matches!(effect, Effect::Stash(StashEffect::PutBack(_))) {
                queue.extend(glowfly_runner::testing::execute_all(effect).into_iter().rev());
            }
        }
        let Some(m) = queue.pop() else { return };
        fx = super::update(st, m, Instant::now());
    }
}

#[test]
fn a_session_ended_before_the_put_back_still_offers_it() {
    let (mut st, dir, fx) = stash_and_checkout("asm-before-put-back");
    finish_but_put_back(&mut st, fx);
    assert_eq!(st.snap.as_ref().unwrap().head_branch.as_deref(), Some("feature"), "the checkout ran");
    assert!(st.auto_stash.pending.iter().any(|p| p.saved && p.oid == Some(top_stash(&dir))), "armed, its record kept");
    drop(st);
    let mut st = offered_at_open(&dir);
    drive(&mut st, Msg::ConfirmAccepted);
    mine_is_back(&st, &dir);
    assert!(!file(&dir).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_conflicted_put_back_removes_the_record() {
    // feature changes a.txt too, so the stashed edit collides with it.
    let (mut st, dir, _) = feature_repo("asm-conflict", &[("feature work", "feature\n")], None);
    git(&dir, &["checkout", "-q", "feature"]);
    std::fs::write(dir.join("a.txt"), "one\nfeature\n").unwrap();
    git(&dir, &["commit", "-q", "-am", "a on feature"]);
    git(&dir, &["checkout", "-q", "main"]);
    std::fs::write(dir.join("a.txt"), "one\nmine\n").unwrap();
    drive(&mut st, Msg::Repo(RepoMsg::Refresh));
    up(&mut st, Msg::git("Checkout", &["checkout", "feature", "--"]));
    super::tests::accept_stash(&mut st);
    assert!(std::fs::read_to_string(dir.join("a.txt")).unwrap().contains("<<<<<<<"), "the put-back conflicted");
    assert_eq!(stash_ops::list(&dir).unwrap().len(), 1, "the stash stays listed");
    assert!(!file(&dir).exists(), "the record is removed");
    drop(st);
    assert!(open(&dir).modal.is_none(), "nothing is offered at the next open");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_put_back_removes_the_record() {
    let (mut st, dir, fx) = stash_and_checkout("asm-failed");
    finish_but_put_back(&mut st, fx);
    let stash = top_stash(&dir);
    assert!(file(&dir).exists());
    let failed = AutoStashMsg::PutBack { workdir: dir.clone(), oid: stash, outcome: PutBack::Failed, output: "boom".into(), undo: None };
    let fx = up(&mut st, Msg::AutoStash(failed));
    assert_eq!(saves(&fx), vec![Vec::<Option<Oid>>::new()], "saved without the record");
    finish(&mut st, fx);
    assert!(!file(&dir).exists());
    assert_eq!(stash_ops::list(&dir).unwrap().len(), 1, "the stash stays in the list");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A folder path that is absolute on every OS (nothing is written there).
fn folder(name: &str) -> PathBuf {
    glowfly_core::paths::temp_dir().join(format!("glowfly-asm-{name}"))
}

fn checkout_job(target: &str) -> AutoStashJob {
    let op = AutoStashFor::Checkout { target: target.into() };
    AutoStashJob { message: op.message(), before: Vec::new(), op }
}

fn making(workdir: &Path, made: Option<Oid>) -> Making {
    Making { label: "Checkout".into(), workdir: Some(workdir.to_path_buf()), job: checkout_job("feature"), made }
}

fn oid(byte: u8) -> Oid {
    Oid::from_bytes(&[byte; 20]).unwrap()
}

fn made(workdir: &Path, oid: Oid) -> Msg {
    Msg::AutoStash(AutoStashMsg::Made { workdir: workdir.to_path_buf(), oid })
}

#[test]
fn made_for_another_repository_changes_nothing() {
    let (a, b) = (folder("made-a"), folder("made-b"));
    let mut st = state();
    st.auto_stash.making = Some(making(&a, None));
    assert!(up(&mut st, made(&b, oid(1))).is_empty());
    assert_eq!(st.auto_stash.making.as_ref().and_then(|m| m.made), None);
    st.auto_stash.making = None;
    assert!(up(&mut st, made(&a, oid(1))).is_empty(), "no job is making a stash");
}

#[test]
fn a_save_while_the_job_runs_keeps_its_record() {
    let a = folder("save-while");
    let mut st = state();
    st.auto_stash.making = Some(making(&a, None));
    let other = AutoStashFor::Pull { branch: "main".into() };
    let stash = SavedAutoStash::new(other.clone(), other.message(), oid(2));
    st.auto_stash.saved.push(SavedOffer { workdir: a.clone(), stash, oid: oid(2), offered: false });
    let fx = up(&mut st, made(&a, oid(1)));
    assert_eq!(saves(&fx), vec![vec![Some(oid(1)), Some(oid(2))]], "the making's record and the one waiting for an answer");
}

#[test]
fn the_running_jobs_record_is_not_offered() {
    let a = folder("not-offered");
    let mut st = state();
    st.auto_stash.making = Some(making(&a, Some(oid(1))));
    let job = checkout_job("feature");
    let saved = vec![SavedAutoStash::new(job.op, job.message, oid(1))];
    up(&mut st, Msg::AutoStash(AutoStashMsg::SavedRead { workdir: a, saved }));
    assert!(st.auto_stash.saved.is_empty(), "the running job's own record waits in memory");
    assert!(st.modal.is_none());
}

#[test]
fn replacing_a_saved_waiting_entry_keeps_the_new_record() {
    let (mut st, dir, fx) = stash_and_checkout("asm-replace");
    // An older auto-stash of this tab still waits, already in the records.
    let old = PendingStash {
        workdir: dir.clone(),
        job: checkout_job("main"),
        oid: Some(oid(7)),
        phase: Phase::PuttingBack,
        told: true,
        saved: true,
    };
    st.auto_stash.pending.push(old);
    let mut msgs = run_the_job(fx);
    let stash = top_stash(&dir);
    let fx = up(&mut st, msgs.remove(0));
    assert_eq!(saves(&fx), vec![vec![Some(stash), Some(oid(7))]]);
    assert!(matches!(&msgs[..], [Msg::Job(JobMsg::JobDone { .. })]), "then the job's result");
    let fx = up(&mut st, msgs.remove(0));
    assert_eq!(saves(&fx).first(), Some(&vec![Some(stash)]), "the new stash, not the old");
    let _ = std::fs::remove_dir_all(&dir);
}

//! Repository trust: git jobs the user starts in a held repository wait for the trust prompt,
//! run on Trust and run, and are dropped on Cancel.

use super::jobs::{JobSpec, Started, begin_job, start_job};
use super::tests::{clean_tree, scratch_repo, state};
use super::trust_tests::{asking, drive, open, real, run, untrusted_repo};
use crate::effect::{Effect, InitEffect, StashEffect};
use crate::msg::{CloneMsg, CommitMsg, InitMsg, Msg, TrustMsg};
use crate::patch::PatchMsg;
use crate::state::clone::CloneRequest;
use crate::state::init::InitRequest;
use crate::state::trust::{TrustPrompt, record, recorded};
use crate::state::{AppState, Modal, RunningJob};
use std::path::PathBuf;
use trellis_core::gitcli::args;
use trellis_core::init_ops::InitPlan;
use trellis_core::settings::RepoTrust;

/// `untrusted_repo` with a clean tree (so pulls and checkouts don't stop at the auto-stash
/// prompt) opened, with the first-open prompt dismissed.
pub(crate) fn held_repo(name: &str) -> (AppState, PathBuf) {
    held_repo_with(name, true)
}

/// `held_repo`, keeping `untrusted_repo`'s uncommitted edit unless `clean`.
pub(crate) fn held_repo_with(name: &str, clean: bool) -> (AppState, PathBuf) {
    let dir = untrusted_repo(name);
    if clean {
        clean_tree(&dir);
    }
    let mut st = state();
    open(&mut st, &dir);
    assert!(asking(&st), "the first open asks");
    drive(&mut st, Msg::CloseModal);
    assert!(st.held());
    (st, dir)
}

fn pull() -> Msg {
    Msg::git("Pull", &["pull", "--ff"])
}

fn push() -> Msg {
    Msg::git("Push", &["push"])
}

fn prompt(st: &AppState) -> &TrustPrompt {
    let Some(Modal::Trust(p)) = &st.modal else { panic!("expected the trust prompt") };
    p
}

fn held_label(st: &AppState) -> Option<&str> {
    st.trust.held.as_ref().map(|h| h.job.label.as_str())
}

fn trust_and_run(st: &mut AppState) -> Vec<Effect> {
    let real = real(st);
    run(st, Msg::Trust(TrustMsg::Decide { real, trusted: true }))
}

fn run_gits(fx: &[Effect]) -> Vec<&Effect> {
    fx.iter().filter(|e| matches!(e, Effect::RunGit { .. })).collect()
}

fn last_log(st: &AppState) -> (bool, &str) {
    let e = st.log.last().expect("a log line");
    (e.ok, e.text.as_str())
}

fn logged(st: &AppState, text: &str) -> bool {
    st.log.iter().any(|e| e.text.contains(text))
}

fn running(label: &str) -> RunningJob {
    RunningJob {
        label: label.into(),
        workdir: None,
        quiet: false,
        clear_commit: false,
        then_push: false,
        then_pr: None,
        then_link: false,
        then_open: None,
        keeps_undo: true,
        fetches: false,
    }
}

#[test]
fn pull_while_undecided_asks_first() {
    let (mut st, dir) = held_repo("held-pull");
    let fx = run(&mut st, pull());
    assert!(fx.is_empty(), "nothing runs: {fx:?}");
    assert!(st.job.is_none());
    assert_eq!(held_label(&st), Some("Pull"));
    let p = prompt(&st);
    assert_eq!(p.held.as_deref(), Some("Pull"));
    assert!(!p.refused);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commit_while_held_asks_first() {
    let (mut st, dir) = held_repo_with("held-commit", false);
    std::fs::write(dir.join("new.txt"), "new\n").unwrap();
    st.draft.summary = "Add a file".into();
    let fx = run(&mut st, Msg::Commit(CommitMsg::StageAllAndCommit));
    assert!(run_gits(&fx).is_empty(), "{fx:?}");
    assert_eq!(prompt(&st).held.as_deref(), Some("Stage all and commit"));
    assert_eq!(st.draft.summary, "Add a file", "the draft is kept");

    let fx = trust_and_run(&mut st);
    let gits = run_gits(&fx);
    let [Effect::RunGit { cmds, write_file, .. }] = gits.as_slice() else { panic!("one RunGit: {fx:?}") };
    assert_eq!(cmds[0], args(&["add", "--all"]));
    assert!(write_file.is_some(), "the commit message file is written");
    let job = st.job.as_ref().expect("the commit runs");
    assert_eq!(job.label, "Stage all and commit");
    assert!(job.clear_commit);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trust_and_run_runs_the_held_job() {
    let (mut st, dir) = held_repo("held-trust-run");
    run(&mut st, pull());
    let real = real(&st);
    let fx = trust_and_run(&mut st);
    assert_eq!(recorded(&st.settings, &real), Some(RepoTrust::Trusted));
    assert!(fx.iter().any(|e| matches!(e, Effect::SaveSettings(_))), "{fx:?}");
    assert!(fx.iter().any(|e| matches!(e, Effect::LoadRepo { opts, .. } if opts.trusted)), "{fx:?}");
    let gits = run_gits(&fx);
    let [Effect::RunGit { cwd, cmds, .. }] = gits.as_slice() else { panic!("one RunGit: {fx:?}") };
    assert_eq!(*cmds, vec![args(&["pull", "--ff"])]);
    assert!(trellis_core::paths::same_spelling(cwd, &st.workdir().unwrap()));
    let job = st.job.as_ref().expect("Pull runs");
    assert_eq!(job.label, "Pull");
    assert!(job.workdir.is_some());
    assert!(st.trust.held.is_none());
    assert!(st.modal.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cancel_runs_nothing() {
    let (mut st, dir) = held_repo("held-cancel");
    run(&mut st, pull());
    st.view.show_log = false;
    let fx = run(&mut st, Msg::CloseModal);
    assert!(fx.is_empty(), "{fx:?}");
    assert!(st.job.is_none());
    assert!(st.trust.held.is_none());
    assert!(st.settings.repo_trust.is_empty(), "nothing is recorded");
    let (ok, text) = last_log(&st);
    assert!(ok && text.contains("Pull wasn't run"), "{text}");
    assert!(!st.view.show_log, "the log doesn't pop open");
    assert!(st.held(), "still undecided");

    run(&mut st, pull());
    assert_eq!(prompt(&st).held.as_deref(), Some("Pull"), "it asks again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dont_trust_drops_the_held_job() {
    let (mut st, dir) = held_repo("held-dont");
    run(&mut st, pull());
    let real = real(&st);
    let fx = run(&mut st, Msg::Trust(TrustMsg::Decide { real: real.clone(), trusted: false }));
    assert_eq!(recorded(&st.settings, &real), Some(RepoTrust::NotTrusted));
    assert!(run_gits(&fx).is_empty(), "{fx:?}");
    assert!(st.job.is_none() && st.trust.held.is_none());
    assert!(logged(&st, "isn't trusted"));
    assert!(logged(&st, "Pull wasn't run"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refused_repository_asks_before_a_job() {
    let dir = untrusted_repo("held-refused");
    clean_tree(&dir);
    let mut st = state();
    open(&mut st, &dir);
    let real = real(&st);
    drive(&mut st, Msg::CloseModal);
    record(&mut st.settings, &real, RepoTrust::NotTrusted);
    assert!(st.held());
    run(&mut st, pull());
    let p = prompt(&st);
    assert!(p.refused);
    assert_eq!(p.held.as_deref(), Some("Pull"));
    let fx = trust_and_run(&mut st);
    assert_eq!(run_gits(&fx).len(), 1, "{fx:?}");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Pull"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_trusted_or_clean_repository_runs_at_once() {
    let clean = scratch_repo("held-clean");
    clean_tree(&clean);
    let mut st = state();
    open(&mut st, &clean);
    assert!(st.modal.is_none());
    let fx = run(&mut st, pull());
    assert_eq!(run_gits(&fx).len(), 1, "{fx:?}");
    assert!(st.trust.held.is_none());
    let _ = std::fs::remove_dir_all(&clean);

    let dir = untrusted_repo("held-trusted");
    clean_tree(&dir);
    let mut st = state();
    let canonical = std::fs::canonicalize(&dir).unwrap();
    record(&mut st.settings, &canonical, RepoTrust::Trusted);
    record(&mut st.settings, &dir, RepoTrust::Trusted);
    open(&mut st, &dir);
    assert!(!st.held(), "trusted before it opened");
    assert!(st.modal.is_none());
    let fx = run(&mut st, pull());
    assert_eq!(run_gits(&fx).len(), 1, "{fx:?}");
    assert!(st.modal.is_none() && st.trust.held.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn clone_and_init_are_not_held() {
    let (mut st, dir) = held_repo("held-init");
    let plan = InitPlan { branch: None, files: Vec::new(), commit: None };
    let fx = run(&mut st, Msg::Init(InitMsg::Start(InitRequest { folder: dir.clone(), plan })));
    let [Effect::Init(InitEffect::Run { folder, .. })] = fx.as_slice() else { panic!("init runs at once: {fx:?}") };
    assert_eq!(folder, &dir);
    assert!(st.trust.held.is_none());
    st.job = None;

    let parent = trellis_core::paths::temp_dir();
    let req = CloneRequest { url: "https://example.invalid/r.git".into(), parent, name: "r".into(), options: Default::default() };
    let fx = run(&mut st, Msg::Clone(CloneMsg::Start(req)));
    let gits = run_gits(&fx);
    let [Effect::RunGit { cmds, .. }] = gits.as_slice() else { panic!("clone runs at once: {fx:?}") };
    assert_eq!(cmds[0][0], "clone");
    assert!(st.trust.held.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_job_in_another_repository_is_not_held() {
    let (mut st, dir) = held_repo("held-other");
    let other = scratch_repo("held-other-clean");
    let mut fx = Vec::new();
    let spec = JobSpec { cwd: Some(other.clone()), ..JobSpec::new("Push".into(), vec![args(&["push"])]) };
    assert_eq!(start_job(&mut st, spec, &mut fx), Started::Now);
    assert_eq!(run_gits(&fx).len(), 1, "{fx:?}");
    assert!(st.trust.held.is_none());
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}

#[test]
fn begin_job_jobs_and_undo_are_held() {
    let (mut st, dir) = held_repo("held-begin");
    let mut fx = Vec::new();
    let effect = |cwd| Effect::Stash(StashEffect::Rename { cwd, index: 0, oid: git2::Oid::zero(), message: "m".into() });
    assert_eq!(begin_job(&mut st, "Rename stash", true, effect, &mut fx), Started::Held);
    assert!(fx.is_empty(), "{fx:?}");
    let fx = trust_and_run(&mut st);
    assert!(fx.iter().any(|e| matches!(e, Effect::Stash(StashEffect::Rename { .. }))), "{fx:?}");
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Rename stash"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn quiet_jobs_are_dropped_without_asking() {
    let (mut st, dir) = held_repo("held-quiet");
    let mut fx = Vec::new();
    let spec = JobSpec { quiet: true, ..JobSpec::new("Fetch".into(), vec![args(&["fetch"])]) };
    assert_eq!(start_job(&mut st, spec, &mut fx), Started::No);
    assert!(fx.is_empty());
    assert!(st.modal.is_none() && st.trust.held.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_second_job_is_refused_while_the_prompt_is_open() {
    let (mut st, dir) = held_repo("held-second");
    run(&mut st, pull());
    let fx = run(&mut st, push());
    assert!(fx.is_empty());
    assert_eq!(held_label(&st), Some("Pull"), "the first job still waits");
    assert!(logged(&st, "Push wasn't run: decide whether to trust"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stale_held_job_is_replaced() {
    let (mut st, dir) = held_repo("held-stale");
    run(&mut st, pull());
    st.modal = Some(Modal::Confirm { title: "t".into(), body: "b".into(), button: "OK".into(), then: Box::new(Msg::CloseModal) });
    run(&mut st, push());
    assert_eq!(held_label(&st), Some("Push"));
    assert!(logged(&st, "Pull wasn't run"));
    assert_eq!(prompt(&st).held.as_deref(), Some("Push"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dropping_clears_job_trackers() {
    let (mut st, dir) = held_repo("held-patch");
    let workdir = st.workdir().unwrap();
    let path = dir.join("fix.patch");
    let how = crate::patch::ApplyHow::Changes(Default::default());
    run(&mut st, Msg::Patch(PatchMsg::Apply(crate::patch::ApplyRequest { workdir, path, how, commits: 0 })));
    assert!(st.trust.held.is_some());
    assert!(st.patch_apply.is_some(), "tracked while held");
    run(&mut st, Msg::CloseModal);
    assert!(st.patch_apply.is_none(), "cleared with the dropped job");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn busy_is_reported_before_holding() {
    let (mut st, dir) = held_repo("held-busy");
    st.job = Some(running("Fetch"));
    let fx = run(&mut st, pull());
    assert!(fx.is_empty());
    assert_eq!(last_log(&st).1, "Another operation is still running.");
    assert!(st.modal.is_none() && st.trust.held.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trust_and_run_drops_the_job_when_busy() {
    let (mut st, dir) = held_repo("held-release-busy");
    run(&mut st, pull());
    st.job = Some(running("Fetch"));
    let fx = trust_and_run(&mut st);
    assert!(run_gits(&fx).is_empty(), "{fx:?}");
    assert!(logged(&st, "Pull wasn't run"));
    assert_eq!(st.job.as_ref().map(|j| j.label.as_str()), Some("Fetch"));
    let _ = std::fs::remove_dir_all(&dir);
}

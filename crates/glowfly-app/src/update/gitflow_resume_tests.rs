//! Headless tests for continuing a Git Flow Finish stopped at a merge conflict: the offer once
//! the merge is committed, the Continue finish job and its headline, a second conflict, the
//! refusals, an aborted merge, parking with the tab, a later Finish clearing the steps, the
//! palette entry, translated log lines and opening another repository, end to end against real
//! repositories.

use super::gitflow_tests::{cleanup, flow, git_job, initialized, job_label, last_log, run};
use super::tests::{drive, finish, git, scratch_repo};
use crate::effect::LoadKind;
use crate::gitflow::GitFlowMsg;
use crate::msg::{JobMsg, Msg, RepoMsg, TabsMsg};
use crate::state::gitflow::FinishJob;
use crate::state::gitflow_resume::StoppedFinish;
use crate::state::palette::{PaletteCommand as C, entries};
use crate::state::{AppState, RunningJob};
use git2::RepositoryState;
use glowfly_core::gitcli::{args, run as git_run};
use glowfly_core::gitflow::{Branches, FinishOptions, StartTips, WorkState, check_finish};
use glowfly_core::i18n::Lang;
use std::path::{Path, PathBuf};

fn commit(dir: &Path, file: &str, text: &str) {
    std::fs::write(dir.join(file), text).unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", &format!("change {file}")]);
}

fn resolve(dir: &Path) {
    std::fs::write(dir.join("a.txt"), "one\nresolved\n").unwrap();
    git(dir, &["add", "a.txt"]);
    git(dir, &["commit", "-q", "--no-edit"]);
}

fn exists(dir: &Path, r: &str) -> bool {
    git_run(dir, &args(&["rev-parse", "--verify", "--quiet", r])).0
}

fn refresh(st: &mut AppState) {
    drive(st, Msg::Repo(RepoMsg::Refresh));
}

fn finish_msg(branch: &str, options: FinishOptions) -> Msg {
    Msg::GitFlow(GitFlowMsg::Finish { branch: branch.into(), options })
}

fn toast(st: &AppState) -> String {
    st.toasts.last().map(|t| t.text.clone()).unwrap_or_default()
}

fn resume(st: &AppState) -> &StoppedFinish {
    st.gitflow_resume.as_ref().expect("a stopped Finish is remembered")
}

fn rest(st: &AppState) -> Vec<String> {
    resume(st).resume.plan.steps.iter().map(|s| s.what.clone()).collect()
}

/// feature/login (from main) and develop change a.txt differently; returns the state with the
/// repository before Finish runs.
fn conflicting(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = initialized(&format!("resume-{name}"));
    git(&dir, &["config", "tag.gpgsign", "false"]);
    git(&dir, &["checkout", "--", "a.txt"]);
    git(&dir, &["checkout", "-q", "develop"]);
    commit(&dir, "a.txt", "one\ndevelop\n");
    git(&dir, &["checkout", "-q", "-b", "feature/login", "main"]);
    commit(&dir, "a.txt", "one\nfeature\n");
    refresh(&mut st);
    (st, dir)
}

/// The same, with Finish stopped at the conflict on develop.
fn stopped(name: &str) -> (AppState, PathBuf) {
    let (mut st, dir) = conflicting(name);
    drive(&mut st, finish_msg("feature/login", FinishOptions::default()));
    assert_eq!(st.snap.as_ref().unwrap().state, RepositoryState::Merge);
    assert_eq!(rest(&st), ["delete feature/login"]);
    (st, dir)
}

#[test]
fn continue_after_commit() {
    let (mut st, dir) = stopped("continue");
    assert!(!entries(&st).iter().any(|e| e.command == C::ContinueGitFlowFinish), "not while merging");
    resolve(&dir);
    refresh(&mut st);
    assert!(resume(&st).ready);
    let e = entries(&st).into_iter().find(|e| e.command == C::ContinueGitFlowFinish).expect("palette entry");
    assert_eq!((e.label.as_str(), e.detail.as_deref()), ("Continue finishing feature/login", Some("1 step left")));

    let fx = flow(&mut st, GitFlowMsg::ContinueFinish);
    let cmds = vec![args(&["checkout", "develop", "--"]), args(&["branch", "-d", "feature/login"])];
    assert_eq!(git_job(&fx), Some(cmds), "back on develop first: HEAD may have moved since the commit");
    assert_eq!(job_label(&st).as_deref(), Some("Continue finish feature login"));
    assert!(st.gitflow_resume.is_some(), "kept until the job ends");
    assert!(!entries(&st).iter().any(|e| e.command == C::ContinueGitFlowFinish), "hidden while a job runs");
    finish(&mut st, fx);
    assert_eq!(toast(&st), "Finished feature login: merged into develop and deleted feature/login.");
    assert!(st.gitflow_resume.is_none());
    assert!(!exists(&dir, "refs/heads/feature/login"));
    cleanup(&dir);
}

#[test]
fn release_two_conflicts() {
    let (mut st, dir) = initialized("resume-release");
    git(&dir, &["config", "tag.gpgsign", "false"]);
    git(&dir, &["checkout", "--", "a.txt"]);
    git(&dir, &["checkout", "-q", "-b", "release/1.2.0", "develop"]);
    commit(&dir, "a.txt", "one\nrelease\n");
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, "a.txt", "one\nmain\n");
    git(&dir, &["checkout", "-q", "develop"]);
    commit(&dir, "a.txt", "one\ndevelop\n");
    refresh(&mut st);
    let options = FinishOptions { tag: "1.2.0".into(), message: "Release 1.2.0".into(), keep: false };
    drive(&mut st, finish_msg("release/1.2.0", options));
    assert_eq!(resume(&st).resume.into, "main");
    assert_eq!(rest(&st), ["tag main as 1.2.0", "check out develop", "merge release/1.2.0 into develop", "delete release/1.2.0"]);

    resolve(&dir);
    refresh(&mut st);
    let e = entries(&st).into_iter().find(|e| e.command == C::ContinueGitFlowFinish).unwrap();
    assert_eq!(e.detail.as_deref(), Some("4 steps left"));
    drive(&mut st, Msg::GitFlow(GitFlowMsg::ContinueFinish));
    assert_eq!(
        toast(&st),
        "Continue finish release 1.2.0 stopped: \"merge release/1.2.0 into develop\" has conflicts. Resolve them, commit the \
         merge, then use Continue finish. Still to do: delete release/1.2.0."
    );
    assert_eq!(resume(&st).resume.into, "develop");
    assert_eq!(rest(&st), ["delete release/1.2.0"]);
    assert!(exists(&dir, "refs/tags/1.2.0"));

    resolve(&dir);
    refresh(&mut st);
    drive(&mut st, Msg::GitFlow(GitFlowMsg::ContinueFinish));
    assert_eq!(toast(&st), "Finished release 1.2.0: merged into main and develop, tagged 1.2.0 and deleted release/1.2.0.");
    assert!(st.gitflow_resume.is_none());
    assert!(!exists(&dir, "refs/heads/release/1.2.0"));
    cleanup(&dir);
}

#[test]
fn continue_refusals() {
    let (mut st, dir) = stopped("refusals");
    let refused = |st: &mut AppState, why: &str| {
        assert!(git_job(&flow(st, GitFlowMsg::ContinueFinish)).is_none(), "{why}");
        assert_eq!(last_log(st), why);
        assert!(st.gitflow_resume.is_some(), "kept: {why}");
    };
    refused(&mut st, "Commit the merge into develop first, then Continue finish.");
    assert!(st.job.is_none());

    resolve(&dir);
    std::fs::write(dir.join("a.txt"), "changed\n").unwrap();
    refresh(&mut st);
    refused(&mut st, "Commit or stash your changes before finishing feature/login.");
    git(&dir, &["checkout", "--", "a.txt"]);
    refresh(&mut st);

    run(&mut st, Msg::git("Status", &["status"]));
    assert!(st.job.is_some());
    refused(&mut st, "Another operation is still running.");
    st.job = None;

    flow(&mut st, GitFlowMsg::ForgetFinish);
    assert!(st.gitflow_resume.is_none());
    flow(&mut st, GitFlowMsg::ContinueFinish);
    assert_eq!(last_log(&st), "There is no stopped finish to continue.");
    cleanup(&dir);
}

#[test]
fn aborted_merge_drops_resume() {
    let (mut st, dir) = conflicting("abort");
    let options = st.snap.as_ref().unwrap().loaded_with.clone();
    let before = glowfly_core::repo::load(&dir, &options).unwrap();
    drive(&mut st, finish_msg("feature/login", FinishOptions::default()));
    // The snapshot right after the job can still be the one from before the merge.
    let current = st.snap.replace(before);
    st.gitflow_resume.as_mut().unwrap().seen_merge = false;
    super::gitflow::snapshot_changed(&mut st);
    assert!(st.gitflow_resume.as_ref().is_some_and(|r| !r.ready), "a stale snapshot is no abort");
    st.snap = current;
    super::gitflow::snapshot_changed(&mut st);
    assert!(resume(&st).seen_merge);

    git(&dir, &["merge", "--abort"]);
    refresh(&mut st);
    assert!(st.gitflow_resume.is_none());
    assert_eq!(
        last_log(&st),
        "Finishing feature/login can't continue: the merge into develop was aborted. Its remaining steps were dropped."
    );
    cleanup(&dir);
}

#[test]
fn moved_branch_drops_resume() {
    let (mut st, dir) = stopped("moved");
    resolve(&dir);
    git(&dir, &["checkout", "-q", "feature/login"]);
    commit(&dir, "b.txt", "more\n");
    refresh(&mut st);
    assert!(st.gitflow_resume.is_none());
    assert_eq!(
        last_log(&st),
        "Finishing feature/login can't continue: feature/login has new commits since the finish stopped. Its remaining steps were dropped."
    );
    cleanup(&dir);
}

/// A Finish job of `plan`-like steps for feature/login, as `finish` sets it up.
fn pending_job(st: &AppState, snap: &glowfly_core::repo::Snapshot, workdir: PathBuf) -> (RunningJob, FinishJob) {
    let s = snap.gitflow.settings().unwrap();
    let clean = WorkState { state: RepositoryState::Clean, tracked_changes: false, elsewhere: vec![] };
    let plan = check_finish(&s, "feature/login", &FinishOptions::default(), &Branches::of(snap), &clean).unwrap();
    let label = plan.label();
    let job = RunningJob {
        label: label.clone(),
        workdir: Some(workdir),
        quiet: false,
        clear_commit: false,
        then_open: None,
        then_push: false,
        then_pr: None,
        then_link: false,
        keeps_undo: false,
        fetches: false,
    };
    assert!(st.gitflow_finish.is_none());
    let done = plan.summary(&s);
    (job, FinishJob { label, tips: StartTips::of(snap, &plan), plan, done, resuming: false })
}

#[test]
fn resume_parks_with_tab() {
    let (mut st, dir) = stopped("park");
    run(&mut st, Msg::Tabs(TabsMsg::NewTab));
    assert!(st.gitflow_resume.is_none());
    let parked = |st: &AppState| st.tabs[0].parked.as_ref().and_then(|p| p.gitflow_resume.clone());
    assert!(parked(&st).is_some());
    run(&mut st, Msg::Tabs(TabsMsg::SelectTab(0)));
    assert_eq!(rest(&st), ["delete feature/login"]);

    // A Finish job of the first tab ends while the second is active.
    let (job, pending) = pending_job(&st, st.snap.as_ref().unwrap(), dir.clone());
    st.gitflow_resume = None;
    run(&mut st, Msg::Tabs(TabsMsg::SelectTab(1)));
    assert!(parked(&st).is_none());
    st.job = Some(job);
    st.gitflow_finish = Some(pending);
    let output = "$ git checkout develop --\n$ git merge --no-ff --no-edit refs/heads/feature/login\n\
                  CONFLICT (content): Merge conflict in a.txt"
        .to_owned();
    run(&mut st, Msg::Job(JobMsg::JobDone { ok: false, output, undo: None }));
    assert!(st.gitflow_resume.is_none(), "not in the active tab");
    let r = parked(&st).expect("stored in the job's tab");
    assert_eq!((r.resume.into.as_str(), r.seen_merge), ("develop", false));
    cleanup(&dir);
}

#[test]
fn successful_finish_clears_same_branch() {
    let (mut st, dir) = initialized("resume-clears");
    git(&dir, &["checkout", "--", "a.txt"]);
    for b in ["feature/x", "feature/login"] {
        git(&dir, &["checkout", "-q", "-b", b, "develop"]);
        commit(&dir, &format!("{}.txt", b.replace('/', "-")), "work\n");
    }
    refresh(&mut st);
    let snap = st.snap.as_ref().unwrap();
    let s = snap.gitflow.settings().unwrap();
    let plan = check_finish(&s, "feature/login", &FinishOptions::default(), &Branches::of(snap), &WorkState::of(snap)).unwrap();
    let r = plan.rest_after_conflict(1, &StartTips::of(snap, &plan).unwrap()).unwrap();
    st.gitflow_resume = Some(StoppedFinish { resume: r, seen_merge: true, ready: false });

    drive(&mut st, finish_msg("feature/x", FinishOptions::default()));
    assert_eq!(toast(&st), "Finished feature x: merged into develop and deleted feature/x.");
    assert!(st.gitflow_resume.is_some(), "another branch's steps stay");
    drive(&mut st, finish_msg("feature/login", FinishOptions::default()));
    assert_eq!(toast(&st), "Finished feature login: merged into develop and deleted feature/login.");
    assert!(st.gitflow_resume.is_none());
    cleanup(&dir);
}

#[test]
fn dropped_reason_is_translated() {
    let (mut st, dir) = stopped("german");
    st.settings.language = "de".into();
    resolve(&dir);
    git(&dir, &["checkout", "-q", "feature/login"]);
    commit(&dir, "b.txt", "more\n");
    refresh(&mut st);
    assert!(st.gitflow_resume.is_none());
    let de = Lang::De;
    let why = de.trf("gitflow.resume.why.branch_moved", &[("branch", &"feature/login")]);
    let expected = de.trf("gitflow.resume.dropped", &[("branch", &"feature/login"), ("why", &why)]);
    assert_eq!(last_log(&st), expected);
    assert!(!last_log(&st).contains("can't continue"), "not the English text");
    cleanup(&dir);
}

#[test]
fn palette_offers_continue_only_when_ready_and_idle() {
    let (mut st, dir) = stopped("palette");
    let offered = |st: &AppState| entries(st).into_iter().find(|e| e.command == C::ContinueGitFlowFinish);
    assert!(offered(&st).is_none(), "not while the merge is open");
    resolve(&dir);
    refresh(&mut st);
    let e = offered(&st).expect("offered once committed");
    assert_eq!(e.detail.as_deref(), Some("1 step left"));
    run(&mut st, Msg::git("Status", &["status"]));
    assert!(st.job.is_some());
    assert!(offered(&st).is_none(), "not while a job runs");
    st.job = None;
    assert!(offered(&st).is_some());
    cleanup(&dir);
}

#[test]
fn opening_another_repository_clears_resume() {
    let (mut st, dir) = stopped("reopen");
    let other = scratch_repo("gitflow-resume-other");
    let options = st.snap.as_ref().unwrap().loaded_with.clone();
    let snap = glowfly_core::repo::load(&other, &options).unwrap();
    let want = snap.workdir.clone();
    let m = Msg::Repo(RepoMsg::RepoLoaded { generation: st.load_gen, kind: LoadKind::Open, result: Ok(Box::new(snap)) });
    run(&mut st, m);
    assert_eq!(st.snap.as_ref().map(|s| &s.workdir), Some(&want), "the other repository is shown");
    assert!(st.gitflow_resume.is_none());
    cleanup(&dir);
    cleanup(&other);
}
